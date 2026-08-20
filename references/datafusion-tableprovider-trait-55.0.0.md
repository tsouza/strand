# Apache DataFusion — `TableProvider` Trait and `MemTable::scan` (tag 55.0.0)

Vendored excerpt, fetched 2026-08-20, grounding roadmap item M5-1's
`StrandLexicalTable` (`crates/strand-datafusion/src/lexical_table.rs`)
against DataFusion's real, current API rather than a remembered or
docs.rs-summarized shape (`CLAUDE.md` §3). This fetch caught a real error
in an earlier docs.rs summary consulted during this task: that summary
invented a required `as_any` trait method and named a `MemoryExec` type
that does not exist at this tag — both corrected against the sources
below before any code was written against them.

**Sources:** `raw.githubusercontent.com/apache/datafusion` at tag
`55.0.0`, paths `datafusion/session/src/table.rs` (the `TableProvider`
trait) and `datafusion/catalog/src/memory/table.rs` (`MemTable`, the
reference in-memory implementation this crate's own `scan()` models
itself on). License: Apache-2.0 (DataFusion is an Apache Software
Foundation project; header confirmed present verbatim in both fetched
files).

## The `TableProvider` trait's real required surface

```rust
pub trait TableProvider: Any + Debug + Sync + Send {
    fn schema(&self) -> SchemaRef;

    fn table_type(&self) -> TableType;

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>>;
}
```

(`constraints`, `get_table_definition`, `get_logical_plan`,
`scan_with_args`, `supports_filters_pushdown`, and others all have default
implementations in the real trait — only `schema`, `table_type`, and
`scan` are genuinely required.)

**No `as_any` method exists on this trait.** The supertrait bound is
`Any + Debug + Sync + Send` — `Any` gives every implementor `type_id()`
for free via the standard library, not a project-defined `as_any`
accessor. A docs.rs-rendered summary consulted before this fetch claimed
`as_any` was a required method; it is not, at this tag.

## `MemTable::scan`, the reference model

```rust
async fn scan(
    &self,
    state: &dyn Session,
    projection: Option<&Vec<usize>>,
    _filters: &[Expr],
    _limit: Option<usize>,
) -> Result<Arc<dyn ExecutionPlan>> {
    let mut partitions = vec![];
    for arc_inner_vec in self.batches.iter() {
        let inner_vec = arc_inner_vec.read().await;
        partitions.push(inner_vec.clone())
    }

    let mut source =
        MemorySourceConfig::try_new(&partitions, self.schema(), projection.cloned())?;
    let show_sizes = state.config_options().explain.show_sizes;
    source = source.with_show_sizes(show_sizes);
    // ... sort-order wiring omitted, not relevant to this crate's scope ...
    Ok(DataSourceExec::from_data_source(source))
}
```

Confirms two real, current types this crate's own `scan()` depends on
directly: `MemorySourceConfig` (`datafusion_datasource::memory`) built
from a `Vec<Vec<RecordBatch>>` of partitions plus the schema and an
optional projection, and `DataSourceExec` (`datafusion_datasource::source`)
wrapping that source into an `ExecutionPlan`. Neither is named
`MemoryExec` — the type an earlier, unvendored docs.rs summary invented —
confirming that summary was wrong at this tag, not merely imprecise.
