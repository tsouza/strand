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
use strand_tools::{ciff, convert, inspect};

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
