# R11(a) — tantivy's reader surface (no codec SPI) and Lucene's codec SPI class surface

Vendored excerpts. Fetched 2026-08-19, against live source (`gh api`/`raw.githubusercontent.com`),
not memory, per `CLAUDE.md` §3.

Cited by: `docs/ledger.md` R11, `docs/milestones.md` M4 (the tantivy-fork second-reader
path and the Lucene `StrandCodec` JVM parity vehicle).

Answers, up front: **tantivy has no codec SPI.** There is no pluggable
postings-format / doc-values-format registration point anywhere in its source tree — no
trait, no registry, no `META-INF/services`-equivalent. The M4 "tantivy fork" path named
in `docs/milestones.md` therefore means literally forking tantivy's source and modifying
its internal readers/writers, not writing a plugin against a stable extension point.
**Lucene does have exactly the SPI this project's `StrandCodec` plan assumed**, built on
`java.util.ServiceLoader` and a closed, fixed abstract surface: `Codec` plus ten
per-structure format classes, one of which (`PostingsFormat`) is the one a `StrandCodec`
must implement to read STRAND's postings from the JVM.

---

## Part 1 — tantivy: no codec SPI, a fixed component enum instead

**Source repo:** `github.com/quickwit-oss/tantivy`. **Pinned:** tag `0.26.1`
(commit `0093923d94157d9f1f63a292bb504bb8db401f2a`, released 2026-05-10 per
`gh api repos/quickwit-oss/tantivy/releases/latest`). Tip of `main` at fetch time was
`fa904b3a7253da0b62d91804cf62be99af15e4ad` (2026-08-19); the tagged release is cited
throughout for stability, matching this project's practice of pinning a release rather
than a moving branch.

### Repo-wide search for a codec/format-registration concept

`gh api "repos/quickwit-oss/tantivy/git/trees/0.26.1?recursive=1" --jq '.tree[].path' | grep -iE "codec|format"`
against the full recursive tree returns exactly one hit outside vendor/test
boilerplate: `columnar/src/columnar/format_version.rs`, a file holding a bare
`u32` format-version constant for the columnar module, not a registration point of any
kind. There is no `src/codecs/`, no `Codec` trait, no `PostingsFormat` trait, no
`ServiceLoader`/plugin-registry equivalent anywhere in the tree. This is the direct,
mechanical answer to "does tantivy have a codec SPI a STRAND-compatible reader could
register through": no.

### `Directory` — a storage abstraction, not a format-plugin point

**Source:** `src/directory/directory.rs` (full file read at the pinned tag).

> ```
> /// Write-once read many (WORM) abstraction for where
> /// tantivy's data should be stored.
> ///
> /// There are currently two implementations of `Directory`
> ///
> /// - The [`MMapDirectory`][crate::directory::MmapDirectory], this should be your default choice.
> /// - The [`RamDirectory`][crate::directory::RamDirectory], which should be used mostly for tests.
> pub trait Directory: DirectoryClone + fmt::Debug + Send + Sync + 'static {
>     fn get_file_handle(&self, path: &Path) -> Result<Arc<dyn FileHandle>, OpenReadError>;
>     fn open_read(&self, path: &Path) -> Result<FileSlice, OpenReadError> { ... }
>     fn delete(&self, path: &Path) -> Result<(), DeleteError>;
>     fn exists(&self, path: &Path) -> Result<bool, OpenReadError>;
>     fn open_write(&self, path: &Path) -> Result<WritePtr, OpenWriteError>;
>     fn atomic_read(&self, path: &Path) -> Result<Vec<u8>, OpenReadError>;
>     fn atomic_write(&self, path: &Path, data: &[u8]) -> io::Result<()>;
>     fn sync_directory(&self) -> io::Result<()>;
>     fn acquire_lock(&self, lock: &Lock) -> Result<DirectoryLock, LockError> { ... }
>     fn watch(&self, watch_callback: WatchCallback) -> crate::Result<WatchHandle>;
> }
> ```

`Directory` is tantivy's analogue of Lucene's own `Directory` class — byte-range file
I/O (open/read/write/delete/lock/watch) against a storage backend (`MmapDirectory`,
`RamDirectory`). It says nothing about how the *bytes inside* a file are structured.
Swapping storage backends (for example, an S3-backed `Directory`) is supported; swapping
the postings encoding is not — there is no method on this trait, or anywhere else in the
crate, that takes a caller-supplied encoder/decoder for a segment component.

### `SegmentComponent` — a closed, seven-variant enum, not a registry

**Source:** `src/index/segment_component.rs` (full file; note the module moved from
`src/core/` to `src/index/` between older tantivy versions and 0.26.1 — itself a small,
concrete demonstration of why `CLAUDE.md` §3 forbids citing this kind of layout from
memory).

> ```rust
> /// Enum describing each component of a tantivy segment.
> #[derive(Copy, Clone, Eq, PartialEq)]
> pub enum SegmentComponent {
>     /// Postings (or inverted list). Sorted lists of document ids, associated with terms
>     Postings,
>     /// Positions of terms in each document.
>     Positions,
>     /// Column-oriented random-access storage of fields.
>     FastFields,
>     /// Stores the sum of the length (in terms) of each field for each document.
>     FieldNorms,
>     /// Dictionary associating `Term`s to `TermInfo`s ...
>     Terms,
>     /// Row-oriented, compressed storage of the documents.
>     Store,
>     /// Bitset describing which document of the segment is alive.
>     Delete,
> }
>
> impl SegmentComponent {
>     pub fn iterator() -> slice::Iter<'static, SegmentComponent> {
>         static SEGMENT_COMPONENTS: [SegmentComponent; 7] = [
>             SegmentComponent::Postings, SegmentComponent::Positions,
>             SegmentComponent::FastFields, SegmentComponent::FieldNorms,
>             SegmentComponent::Terms, SegmentComponent::Store, SegmentComponent::Delete,
>         ];
>         SEGMENT_COMPONENTS.iter()
>     }
> }
> ```

Seven fixed file kinds, no eighth slot a third party can register. Compare Lucene's
`Codec`, below, whose ten format methods are each independently *replaceable* by
subclassing.

### `SegmentReader` and `InvertedIndexReader` — concrete types wired to concrete formats

**Source:** `src/index/segment_reader.rs` (`SegmentReader::open_with_custom_alive_set`,
lines ~148–190 at the pinned tag) opens each `SegmentComponent` by hardcoded match on
the enum above and constructs the one reader type tantivy ships for it:
`CompositeFile::open` over `SegmentComponent::Terms`/`Postings`/`Positions`,
`FastFieldReaders::open` over `SegmentComponent::FastFields`,
`FieldNormReaders::open` over `SegmentComponent::FieldNorms`,
`StoreReader::open` (lazily, via `get_store_reader`) over `SegmentComponent::Store`.
None of these calls take a format/codec parameter; the types are fixed at compile time.

**Source:** `src/index/inverted_index_reader.rs`:

> ```rust
> use crate::postings::{BlockSegmentPostings, SegmentPostings, TermInfo};
> ...
> pub struct InvertedIndexReader {
>     termdict: TermDictionary,
>     postings_file_slice: FileSlice,
>     positions_file_slice: FileSlice,
>     record_option: IndexRecordOption,
>     total_num_tokens: u64,
> }
> ```

`TermDictionary` (an FST-backed sstable type from the `sstable`/`tantivy-fst` crates),
`BlockSegmentPostings`, and `SegmentPostings` are concrete structs, not trait objects
behind a format registry. `src/postings/postings.rs` does define a `Postings` trait —

> ```rust
> /// Postings (also called inverted list)
> /// ...
> /// Its main implementation is `SegmentPostings`,
> /// but other implementations mocking `SegmentPostings` exist,
> /// for merging segments or for testing.
> pub trait Postings: DocSet + 'static {
>     fn term_freq(&self) -> u32;
>     fn append_positions_with_offset(&mut self, offset: u32, output: &mut Vec<u32>);
>     ...
> }
> ```

— but this is a **runtime query-result iterator interface** (the analogue of Lucene's
`PostingsEnum`/`DocIdSetIterator`: "give me the next doc id, its term frequency, its
positions"), evaluated over postings tantivy has already decoded. It is not a wire-format
registration point: nothing implements `Postings` by decoding a caller-supplied on-disk
byte layout, and nothing in `InvertedIndexReader` accepts a `Box<dyn Postings>`-producing
factory in place of `BlockSegmentPostings`. The distinction matters because a superficial
grep for "trait ... Postings" could be mistaken for a codec SPI; reading the
implementation confirms it is not one.

### The one real (narrow) knob: doc-store compression, not a format registry

**Source:** `src/store/compressors.rs`:

> ```rust
> /// Compressor can be used on `IndexSettings` to choose
> /// the compressor used to compress the doc store.
> /// The default is Lz4Block, but also depends on the enabled feature flags.
> #[derive(Clone, Debug, Copy, PartialEq, Eq)]
> pub enum Compressor {
>     /// No compression
>     None,
>     #[cfg(feature = "lz4-compression")]
>     Lz4,
>     #[cfg(feature = "zstd-compression")]
>     Zstd(ZstdCompressor),
> }
> ```

A closed, three-variant enum selectable at index-build time via `IndexSettings`, gated
by Cargo feature flags at compile time. This is a configuration knob for the one
component (the row-oriented doc store) where tantivy already ships more than one
built-in compressor — it is not an open registration surface a downstream crate can
extend with a fourth, external compressor, let alone with a different postings or
fast-field layout. It does not change the answer for postings, positions, the term
dictionary, or fast fields, which have exactly one built-in encoding each.

### What this settles for R11(a)'s tantivy half

`docs/ledger.md`'s open item "tantivy codec-SPI absence" is now a confirmed finding,
not a hypothesis: tantivy exposes a storage abstraction (`Directory`) and a
runtime-iterator trait (`Postings`), neither of which is a wire-format plugin point, and
its on-disk layout is a fixed seven-component enum with one concrete reader/writer pair
per component, compiled into the crate. A "STRAND-compatible reader riding inside
tantivy" is not achievable by implementing a trait and registering it; it requires
forking `quickwit-oss/tantivy` and modifying `src/index/segment_reader.rs`,
`src/index/inverted_index_reader.rs`, `src/index/segment_component.rs`, and the
concrete `postings`/`termdict`/`fastfield`/`store` modules directly — exactly the "fork,
not plugin" characterization `docs/milestones.md` M4 already uses ("The tantivy fork is
the named primary second-reader path"), now confirmed against current source rather than
assumed.

---

## Part 2 — Lucene: the real codec SPI, exact class surface

**Source repo:** `github.com/apache/lucene`. **Pinned:** tag `releases/lucene/10.5.1`
(commit `6bde4304bc737c28212cbae91400a62844834b73`, published 2026-08-12 per
`gh api repos/apache/lucene/releases/latest`) — current at fetch time (2026-08-19).

### `Codec` — the top-level SPI class

**Source:** `lucene/core/src/java/org/apache/lucene/codecs/Codec.java`, full file read
at the pinned tag.

> ```java
> /**
>  * Encodes/decodes an inverted index segment.
>  *
>  * <p>Note, when extending this class, the name ({@link #getName}) is written into the index. In
>  * order for the segment to be read, the name must resolve to your implementation via {@link
>  * #forName(String)}. This method uses Java's {@link ServiceLoader Service Provider Interface} (SPI)
>  * to resolve codec names.
>  *
>  * <p>If you implement your own codec, make sure that it has a no-arg constructor so SPI can load
>  * it.
>  */
> public abstract class Codec implements NamedSPILoader.NamedSPI {
>   ...
>   private static final class Holder {
>     private static final NamedSPILoader<Codec> LOADER = new NamedSPILoader<>(Codec.class);
>     @SuppressWarnings("NonFinalStaticField")
>     static Codec defaultCodec = LOADER.lookup("Lucene104");
>   }
>
>   protected Codec(String name) { ... }
>   public final String getName() { return name; }
>
>   public abstract PostingsFormat postingsFormat();
>   public abstract DocValuesFormat docValuesFormat();
>   public abstract StoredFieldsFormat storedFieldsFormat();
>   public abstract TermVectorsFormat termVectorsFormat();
>   public abstract FieldInfosFormat fieldInfosFormat();
>   public abstract SegmentInfoFormat segmentInfoFormat();
>   public abstract NormsFormat normsFormat();
>   public abstract LiveDocsFormat liveDocsFormat();
>   public abstract CompoundFormat compoundFormat();
>   public abstract PointsFormat pointsFormat();
>   public abstract KnnVectorsFormat knnVectorsFormat();
>
>   public static Codec forName(String name) { return Holder.getLoader().lookup(name); }
>   public static Set<String> availableCodecs() { return Holder.getLoader().availableServices(); }
>   public static void reloadCodecs(ClassLoader classloader) { Holder.getLoader().reload(classloader); }
>   public static Codec getDefault() { ... }
>   public static void setDefault(Codec codec) { ... }
> }
> ```

Confirms: (1) the current (10.5.1) default codec name is **`Lucene104`**, resolved
through the same SPI lookup a custom codec would use; (2) the exact abstract surface a
`StrandCodec` implementation must satisfy is these **eleven** format-returning methods,
not just `postingsFormat()` — a `StrandCodec` that wants to be a first-class `Codec`
(installable as the *default* for an index, not merely a per-field format) must supply
all eleven, though most can delegate to an existing Lucene codec's implementations (see
`FilterCodec` below) and only `postingsFormat()` (and, if STRAND vector blobs are also
exposed to Lucene, `knnVectorsFormat()`) need genuinely new STRAND-backed logic.

### `PostingsFormat` — the class STRAND's postings must actually implement

**Source:** `lucene/core/src/java/org/apache/lucene/codecs/PostingsFormat.java`, full
file read at the pinned tag.

> ```java
> /**
>  * Encodes/decodes terms, postings, and proximity data.
>  *
>  * <p>Note, when extending this class, the name ({@link #getName}) may written into the index in
>  * certain configurations. In order for the segment to be read, the name must resolve to your
>  * implementation via {@link #forName(String)}. This method uses Java's {@link ServiceLoader Service
>  * Provider Interface} (SPI) to resolve format names.
>  *
>  * <p>If you implement your own format, make sure that it has a no-arg constructor so SPI can load
>  * it.
>  */
> public abstract class PostingsFormat implements NamedSPILoader.NamedSPI {
>   protected PostingsFormat(String name) { ... }
>   public final String getName() { return name; }
>
>   /** Writes a new segment */
>   public abstract FieldsConsumer fieldsConsumer(SegmentWriteState state) throws IOException;
>
>   /**
>    * Reads a segment. NOTE: by the time this call returns, it must hold open any files it will need
>    * to use; else, those files may be deleted. ... IOExceptions are expected and will
>    * automatically cause a retry of the segment opening logic with the newly revised segments.
>    */
>   public abstract FieldsProducer fieldsProducer(SegmentReadState state) throws IOException;
>
>   public static PostingsFormat forName(String name) { return Holder.getLoader().lookup(name); }
>   public static Set<String> availablePostingsFormats() { return Holder.getLoader().availableServices(); }
>   public static void reloadPostingsFormats(ClassLoader classloader) { Holder.getLoader().reload(classloader); }
> }
> ```

A `StrandCodec`'s lexical half reduces to implementing `FieldsProducer` (read path: term
enumeration, `PostingsEnum`/doc-id iteration, positions) over STRAND's own postings blob
and `FieldsConsumer` (write path, needed only if `StrandCodec` also writes Lucene
segments, not required for a read-only parity vehicle) — a much smaller surface than
reimplementing all eleven `Codec` methods from scratch, because `PostingsFormat` is
independently pluggable.

### `KnnVectorsFormat` — the vector-side analogue

**Source:** `lucene/core/src/java/org/apache/lucene/codecs/KnnVectorsFormat.java`
(header read at the pinned tag):

> ```java
> /**
>  * Encodes/decodes per-document vector and any associated indexing structures required to support
>  * nearest-neighbor search
>  */
> public abstract class KnnVectorsFormat implements NamedSPILoader.NamedSPI {
>   public static final int DEFAULT_MAX_DIMENSIONS = 1024;
>   protected KnnVectorsFormat(String name) { ... }
>   ...
> }
> ```

Same `NamedSPILoader`/`ServiceLoader` mechanism, same per-format independence — relevant
if a future `StrandCodec` also exposes STRAND's RaBitQ vector blobs to Lucene/DataFusion
parity testing, though `docs/milestones.md` scopes M4's `StrandCodec` to lexical parity
specifically.

### `FilterCodec` — the actual construction pattern for a StrandCodec

**Source:** `lucene/core/src/java/org/apache/lucene/codecs/FilterCodec.java`, full file
read at the pinned tag:

> ```java
> /**
>  * A codec that forwards all its method calls to another codec.
>  *
>  * <p>Extend this class when you need to reuse the functionality of an existing codec. For example,
>  * if you want to build a codec that redefines LuceneMN's {@link LiveDocsFormat}:
>  *
>  * <pre class="prettyprint">
>  *   public final class CustomCodec extends FilterCodec {
>  *     public CustomCodec() {
>  *       super("CustomCodec", new LuceneMNCodec());
>  *     }
>  *     public LiveDocsFormat liveDocsFormat() {
>  *       return new CustomLiveDocsFormat();
>  *     }
>  *   }
>  * </pre>
>  *
>  * <p><em>Please note:</em> Don't call {@link Codec#forName} from the no-arg constructor of your own
>  * codec. When the SPI framework loads your own Codec as SPI component, SPI has not yet fully
>  * initialized! If you want to extend another Codec, instantiate it directly by calling its
>  * constructor.
>  */
> public abstract class FilterCodec extends Codec {
>   protected final Codec delegate;
>   protected FilterCodec(String name, Codec delegate) { super(name); this.delegate = delegate; }
>   @Override public DocValuesFormat docValuesFormat() { return delegate.docValuesFormat(); }
>   @Override public FieldInfosFormat fieldInfosFormat() { return delegate.fieldInfosFormat(); }
>   @Override public LiveDocsFormat liveDocsFormat() { return delegate.liveDocsFormat(); }
>   @Override public NormsFormat normsFormat() { return delegate.normsFormat(); }
>   @Override public PostingsFormat postingsFormat() { return delegate.postingsFormat(); }
>   @Override public SegmentInfoFormat segmentInfoFormat() { return delegate.segmentInfoFormat(); }
>   @Override public StoredFieldsFormat storedFieldsFormat() { return delegate.storedFieldsFormat(); }
>   @Override public TermVectorsFormat termVectorsFormat() { return delegate.termVectorsFormat(); }
>   @Override public CompoundFormat compoundFormat() { return delegate.compoundFormat(); }
>   @Override public PointsFormat pointsFormat() { return delegate.pointsFormat(); }
>   @Override public KnnVectorsFormat knnVectorsFormat() { return delegate.knnVectorsFormat(); }
> }
> ```

This is the exact, Apache-documented recipe for `StrandCodec`: `extends FilterCodec`,
constructor `super("StrandCodec", new Lucene104Codec())` (or whichever concrete codec is
current at implementation time — `Lucene104Codec` is the 10.5.1 default, confirmed
above), override only `postingsFormat()` to return a STRAND-backed `PostingsFormat`,
delegate the other ten methods. `Lucene104Codec.java`
(`lucene/core/src/java/org/apache/lucene/codecs/lucene104/Lucene104Codec.java`, header
read at the pinned tag) confirms this is exactly how Lucene's own current default codec
is assembled — it composes per-structure formats from `lucene90`/`lucene94`/`lucene99`
packages (`Lucene90CompoundFormat`, `Lucene99HnswVectorsFormat`,
`Lucene94FieldInfosFormat`, etc.) behind `PerFieldPostingsFormat` /
`PerFieldDocValuesFormat` / `PerFieldKnnVectorsFormat` wrappers, the standard pattern for
letting different fields use different formats within one codec — the same pattern a
`StrandCodec` would use if only some fields are STRAND-backed.

### The registration mechanism: `META-INF/services`, confirmed by an actual file

**Source:** `lucene/core/src/resources/META-INF/services/org.apache.lucene.codecs.Codec`,
full file content at the pinned tag:

> ```
> org.apache.lucene.codecs.lucene104.Lucene104Codec
> ```

This is a real, currently-shipping Java `ServiceLoader` provider-configuration file:
one fully-qualified class name per line, in a file named for the SPI interface
(`org.apache.lucene.codecs.Codec`), under `META-INF/services/` on the classpath. A
`StrandCodec` jar registers itself identically: a file at
`META-INF/services/org.apache.lucene.codecs.Codec` containing
`org.apache.strand.lucene.StrandCodec` (or whatever fully-qualified name is chosen), plus
— if `StrandCodec`'s `postingsFormat()` needs independent discovery outside a full-codec
install (e.g. under `PerFieldPostingsFormat`) — a second file at
`META-INF/services/org.apache.lucene.codecs.PostingsFormat` naming the
`PostingsFormat` subclass directly. Confirmed present at the same pinned tag:
`lucene/core/src/resources/META-INF/services/org.apache.lucene.codecs.PostingsFormat`,
`.../org.apache.lucene.codecs.DocValuesFormat`, and
`.../org.apache.lucene.codecs.KnnVectorsFormat` (verified to exist by directory listing;
contents not individually re-fetched since the `Codec` file's format is representative
and each SPI file class self-documents the same one-class-per-line format).

**Source:** `lucene/core/src/java/org/apache/lucene/util/NamedSPILoader.java` (header
and `reload()` method read at the pinned tag) confirms the loader is a thin wrapper over
the JDK's own mechanism, not a Lucene-invented one:

> ```java
> /**
>  * Helper class for loading named SPIs from classpath (e.g. Codec, PostingsFormat).
>  */
> public final class NamedSPILoader<S extends NamedSPILoader.NamedSPI> implements Iterable<S> {
>   ...
>   public void reload(ClassLoader classloader) {
>     ...
>     for (final S service : ServiceLoader.load(clazz, classloader)) {
>       final String name = service.getName();
>       // only add the first one for each name, later services will be ignored
>       ...
>     }
>   }
> }
> ```

`java.util.ServiceLoader.load(Codec.class, classloader)` is the actual mechanism;
`NamedSPILoader` only adds the name-to-instance indirection (`Codec`/`PostingsFormat`
instances additionally expose a `getName()` string, checked "all ascii alphanumeric,
less than 128 characters," because that name — not the Java class name — is what gets
written into the Lucene segment so a later JVM can look the format back up via
`Codec.forName(name)` / `PostingsFormat.forName(name)` without needing the original
class on an identical classpath layout).

### What this settles for R11(a)'s Lucene half

The exact class surface a future `StrandCodec` needs, current as of Lucene 10.5.1
(2026-08-12): extend `org.apache.lucene.codecs.FilterCodec` (or `Codec` directly, if all
eleven formats are STRAND-native), override `postingsFormat()` to return a
`org.apache.lucene.codecs.PostingsFormat` subclass whose `fieldsProducer(SegmentReadState)`
constructs a `FieldsProducer` reading STRAND's own postings/term-dictionary blobs, and
register the concrete class name in
`META-INF/services/org.apache.lucene.codecs.Codec` (and, if the `PostingsFormat` needs
independent per-field discovery, `META-INF/services/org.apache.lucene.codecs.PostingsFormat`
too), one fully-qualified class name per line, exactly as Lucene's own
`Lucene104Codec` / `org.apache.lucene.codecs.Codec` file already demonstrates. This
confirms the SPI this project's `StrandCodec` plan assumed genuinely exists in current
Lucene, with `java.util.ServiceLoader` — not a Lucene-private mechanism — underneath it.
