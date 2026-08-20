// Copyright the STRAND authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use strand_tools::{ciff, convert, inspect, orphan_sweep};

#[derive(Parser)]
#[command(name = "strand-tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Decode a segment file's footer and hotcache and print a report.
    Inspect { path: PathBuf },
    /// Import one field of a real tantivy index into a real STRAND segment
    /// file (single-segment, deletion-free tantivy indexes only).
    Convert {
        /// Path to the tantivy index directory (containing meta.json).
        #[arg(long = "index-dir")]
        index_dir: PathBuf,
        /// Name of the tantivy text field to import.
        #[arg(long)]
        field: String,
        /// Path to write the resulting STRAND segment file to.
        #[arg(long)]
        output: PathBuf,
    },
    /// Import a real CIFF (Common Index File Format) export into a real
    /// STRAND segment file (lossless where CIFF permits — see
    /// `strand_tools::ciff`'s own module documentation for exactly what
    /// is and is not preserved).
    ConvertCiff {
        /// Path to the `.ciff` file.
        #[arg(long = "ciff-file")]
        ciff_file: PathBuf,
        /// Name of the STRAND field this CIFF export's postings become
        /// (CIFF itself has no field concept).
        #[arg(long)]
        field: String,
        /// Path to write the resulting STRAND segment file to.
        #[arg(long)]
        output: PathBuf,
    },
    /// The M3-5 orphan sweep (`spec/manifest.md` "Orphan files"): list a
    /// table's real object-storage prefix, subtract everything referenced
    /// by its retained snapshots, and delete the remainder that is older
    /// than the retention window.
    Sweep {
        /// S3-compatible bucket the table lives in.
        #[arg(long)]
        bucket: String,
        /// Key prefix under which this table's `_strand/` metadata,
        /// segments, and deletion vectors all live. Defaults to the bucket
        /// root — the convention every existing test in this repository
        /// uses (one table per bucket). Pass an explicit prefix for a
        /// bucket that hosts more than one table.
        #[arg(long, default_value = "")]
        prefix: String,
        /// Override endpoint (MinIO, or any non-AWS S3-compatible store).
        /// When set, requests use path-style addressing, matching MinIO's
        /// requirement (`crates/strand-core/tests/s3_store.rs`).
        #[arg(long = "endpoint-url")]
        endpoint_url: Option<String>,
        /// AWS region. Falls back to the standard SDK provider chain
        /// (environment, profile, IMDS) when omitted.
        #[arg(long)]
        region: Option<String>,
        /// How long an unreferenced object must sit before it is eligible
        /// for deletion — the safety margin against a race with a commit
        /// still in flight (`spec/manifest.md` "Orphan files"). Defaults
        /// to Apache Iceberg's own `remove_orphan_files` default of 3
        /// days (`references/iceberg-remove-orphan-files-procedure.md`).
        #[arg(
            long = "retention-window-secs",
            default_value_t = orphan_sweep::DEFAULT_RETENTION_WINDOW_MILLIS / 1000
        )]
        retention_window_secs: u64,
        /// Report what would be deleted without deleting anything.
        #[arg(long = "dry-run")]
        dry_run: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Inspect { path } => run_inspect(&path),
        Command::Convert {
            index_dir,
            field,
            output,
        } => run_convert(&index_dir, &field, &output),
        Command::ConvertCiff {
            ciff_file,
            field,
            output,
        } => run_convert_ciff(&ciff_file, &field, &output),
        Command::Sweep {
            bucket,
            prefix,
            endpoint_url,
            region,
            retention_window_secs,
            dry_run,
        } => run_sweep(
            &bucket,
            &prefix,
            endpoint_url.as_deref(),
            region.as_deref(),
            retention_window_secs,
            dry_run,
        ),
    }
}

fn run_convert_ciff(
    ciff_file: &std::path::Path,
    field: &str,
    output: &std::path::Path,
) -> ExitCode {
    let (field_blobs, row_count) = match ciff::import_ciff(ciff_file, field) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut builder = strand_core::segment::SegmentBuilder::new(row_count);
    for blob in field_blobs.to_blob_specs() {
        builder.add_blob(blob);
    }
    let segment_bytes = builder.build(0);

    match std::fs::write(output, &segment_bytes) {
        Ok(()) => {
            // TERM_INFO_RECORD_LEN's short variant: import_ciff always
            // calls build_field_from_postings with with_positions =
            // false (CIFF cannot supply positions — ciff.rs's own module
            // doc comment), so the short, 16-byte-per-term record shape
            // is always what gets written here.
            println!(
                "wrote {} ({} bytes, {row_count} rows, {} terms)",
                output.display(),
                segment_bytes.len(),
                field_blobs.term_info.len()
                    / strand_lexical::term_dictionary::SHORT_TERM_INFO_RECORD_LEN,
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: could not write {}: {e}", output.display());
            ExitCode::FAILURE
        }
    }
}

fn run_sweep(
    bucket: &str,
    prefix: &str,
    endpoint_url: Option<&str>,
    region: Option<&str>,
    retention_window_secs: u64,
    dry_run: bool,
) -> ExitCode {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: could not start an async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };

    let client = runtime.block_on(async {
        let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest());
        if let Some(region) = region {
            loader = loader.region(aws_config::Region::new(region.to_string()));
        }
        if let Some(endpoint_url) = endpoint_url {
            loader = loader.endpoint_url(endpoint_url);
        }
        let config = loader.load().await;
        let mut s3_config = aws_sdk_s3::config::Builder::from(&config);
        if endpoint_url.is_some() {
            // MinIO (and most non-AWS S3-compatible stores) require
            // path-style addressing — the same setting
            // `tests/s3_store.rs`'s `start_store` uses.
            s3_config = s3_config.force_path_style(true);
        }
        aws_sdk_s3::Client::from_conf(s3_config.build())
    });

    let store = strand_core::s3_store::S3Store::new(client, bucket);
    let retention_window_millis = retention_window_secs.saturating_mul(1000);
    let now_millis = orphan_sweep::now_millis();

    let outcome = match orphan_sweep::sweep_orphans(
        &store,
        prefix,
        retention_window_millis,
        now_millis,
        dry_run,
    ) {
        Ok(outcome) => outcome,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let verb = if dry_run { "would delete" } else { "deleted" };
    println!(
        "{verb} {} object(s), retained {} referenced object(s), skipped {} object(s) within the retention window",
        outcome.deleted.len(),
        outcome.retained_referenced.len(),
        outcome.skipped_within_window.len(),
    );
    for key in &outcome.deleted {
        println!("  {verb}: {key}");
    }
    for key in &outcome.skipped_within_window {
        println!("  skipped (within retention window): {key}");
    }

    ExitCode::SUCCESS
}

fn run_convert(index_dir: &std::path::Path, field: &str, output: &std::path::Path) -> ExitCode {
    let (field_blobs, row_count) = match convert::import_tantivy_field(index_dir, field) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut builder = strand_core::segment::SegmentBuilder::new(row_count);
    for blob in field_blobs.to_blob_specs() {
        builder.add_blob(blob);
    }
    let segment_bytes = builder.build(0);

    match std::fs::write(output, &segment_bytes) {
        Ok(()) => {
            // TERM_INFO_RECORD_LEN (28 bytes), not the short variant: the
            // importer always calls build_field_from_postings with
            // with_positions = true (convert.rs's own stated scope).
            println!(
                "wrote {} ({} bytes, {row_count} rows, {} terms)",
                output.display(),
                segment_bytes.len(),
                field_blobs.term_info.len() / strand_lexical::term_dictionary::TERM_INFO_RECORD_LEN,
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: could not write {}: {e}", output.display());
            ExitCode::FAILURE
        }
    }
}

fn run_inspect(path: &std::path::Path) -> ExitCode {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("error: could not read {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    };
    match inspect::format_report(&bytes) {
        Ok(report) => {
            print!("{report}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
