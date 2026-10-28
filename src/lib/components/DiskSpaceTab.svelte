<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { FolderOpen, Play, Square, HardDrive, File, Folder, Search, X, ArrowUp, ArrowDown, Upload, Save, History } from "@lucide/svelte";
  import { toast } from "../toast.svelte";
  import { theme } from "../theme.svelte";
  import { t } from "$lib/i18n.svelte";

  interface DiskTreeNode {
    name: string;
    path: string;
    is_dir: boolean;
    size_bytes: number;
    allocated_bytes: number;
    modified_epoch_ms: number | null;
    attributes: string;
    file_count: number;
    children: DiskTreeNode[];
  }

  interface DiskFile {
    name: string;
    path: string;
    extension: string;
    size_bytes: number;
    allocated_bytes: number;
    modified_epoch_ms: number | null;
    attributes: string;
  }

  interface FileTypeStat {
    extension: string;
    size_bytes: number;
    file_count: number;
  }

  interface DuplicateGroup {
    name: string;
    size_bytes: number;
    file_count: number;
    files: DiskFile[];
  }

  interface DiskScanResult {
    root_path: string;
    root: DiskTreeNode;
    top_files: DiskFile[];
    file_types: FileTypeStat[];
    duplicate_groups: DuplicateGroup[];
    total_files: number;
    total_dirs: number;
    total_size: number;
    total_allocated: number;
    volume_total_bytes: number;
    volume_free_bytes: number;
    elapsed_ms: number;
    errors: number;
    truncated: boolean;
    engine: "mft" | "traversal" | "csv" | "multi" | "mft-dump";
    duplicate_mode: "off" | "name_size" | "name_size_date";
    protected_roots?: string[];
  }

  interface DiskProgress {
    current_path: string;
    files: number;
    dirs: number;
    bytes: number;
  }

  interface DiskFileSearchResult {
    files: DiskFile[];
    total_matches: number;
    indexed_files: number;
    index_complete: boolean;
  }

  interface DiskCsvExport {
    csv: string;
    exported: number;
    total: number;
    truncated: boolean;
  }

  interface DriveInfo {
    letter: string;
    label: string;
    kind: "fixed" | "removable";
    total_bytes: number;
    free_bytes: number;
  }

  interface MftDumpInfo {
    path: string;
    name: string;
    size_bytes: number;
    modified_epoch_ms: number | null;
  }

  interface MftDumpResponse {
    result: DiskScanResult;
    dump_path: string;
  }

  interface LayoutRect {
    node: DiskTreeNode;
    x: number;
    y: number;
    w: number;
    h: number;
    depth: number;
  }

  const DISK_PREFS_KEY = "purgekit.disk.preferences.v1";

  let pathInput = $state("C:\\");
  let drives = $state<DriveInfo[]>([]);
  let result = $state<DiskScanResult | null>(null);
  let focusNode = $state<DiskTreeNode | null>(null);
  let focusStack = $state<DiskTreeNode[]>([]);
  let selectedNode = $state<DiskTreeNode | null>(null);
  let activeView = $state<"treemap" | "files" | "types" | "duplicates">("treemap");
  let duplicateMode = $state<"off" | "name_size" | "name_size_date">("off");
  let lastScanHadDuplicates = $state(false);
  let isScanning = $state(false);
  let isCancelling = $state(false);
  let isImporting = $state(false);
  let csvInputEl = $state<HTMLInputElement | null>(null);
  let isAdmin = $state(false);
  let showMftDumps = $state(false);
  let mftDumps = $state<MftDumpInfo[]>([]);
  let isLoadingMftDumps = $state(false);
  let loadingDumpPath = $state<string | null>(null);
  let progress = $state<DiskProgress | null>(null);
  let errorMessage = $state("");
  let canvasEl = $state<HTMLCanvasElement | null>(null);
  let layoutRects: LayoutRect[] = [];
  let hoverPath = $state<string | null>(null);
  let tooltip = $state<{ x: number; y: number; node: DiskTreeNode } | null>(null);
  let fileQuery = $state("");
  let fileExtensionFilter = $state<string | null>(null);
  let fileSort = $state<"size" | "modified" | "name">("size");
  let fileScope = $state<"all" | "folder">("all");
  let fileFilterMode = $state<"name" | "path">("name");
  let fileInclude = $state("");
  let fileExclude = $state("");
  let fileLimit = $state(500);
  let treemapShowNames = $state(true);
  let treemapUseLogical = $state(false);
  let treemapShowFree = $state(false);
  let treemapGroupSmall = $state(0);
  let searchResult = $state<DiskFileSearchResult | null>(null);
  let isSearching = $state(false);
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let searchSequence = 0;
  let isQuarantining = $state(false);
  let duplicateSelection = $state<Record<string, boolean>>({});

  let scanTargets = $derived(
    pathInput
      .split(/[;\n]+/)
      .map((value) => value.trim())
      .filter(Boolean)
  );

  let primaryScanTarget = $derived(scanTargets[0] ?? "");

  let isSnapshotView = $derived(
    result?.engine === "csv" || result?.engine === "mft-dump"
  );

  let selectedDuplicateCount = $derived(
    result
      ? result.duplicate_groups.reduce(
          (count, group) =>
            count + group.files.filter((file) => duplicateSelection[file.path]).length,
          0
        )
      : 0
  );

  let selectedDuplicateBytes = $derived(
    result
      ? result.duplicate_groups.reduce(
          (sum, group) =>
            sum +
            group.files.reduce(
              (groupSum, file) =>
                duplicateSelection[file.path]
                  ? groupSum + Math.max(file.allocated_bytes, file.size_bytes)
                  : groupSum,
              0
            ),
          0
        )
      : 0
  );

  let filteredTopFiles = $derived(
    result
      ? result.top_files.filter((file) => {
          const query = fileQuery.trim().toLowerCase();
          const path = file.path.toLowerCase();
          const include = fileInclude.trim().toLowerCase();
          const exclude = fileExclude.trim().toLowerCase();
          const haystack = fileFilterMode === "path" ? path : file.name.toLowerCase();
          if (fileExtensionFilter && file.extension.toLowerCase() !== fileExtensionFilter.toLowerCase()) {
            return false;
          }
          if (include && !path.includes(include)) return false;
          if (exclude && path.includes(exclude)) return false;
          return !query ||
            haystack.includes(query) ||
            file.extension.toLowerCase().includes(query);
        })
      : []
  );

  let scanRootWeight = $derived(result ? nodeWeight(result.root) : 0);

  let duplicateStats = $derived(
    buildDuplicateStats(result?.duplicate_groups ?? [])
  );

  function buildDuplicateStats(groups: DuplicateGroup[]): Map<string, { count: number; waste: number }> {
    const stats = new Map<string, { count: number; waste: number }>();
    for (const group of groups) {
      const waste = group.size_bytes * (group.file_count - 1);
      for (const file of group.files) {
        stats.set(file.path, { count: group.file_count, waste });
      }
    }
    return stats;
  }

  function nodeDisplayPath(node: DiskTreeNode | null | undefined): string {
    if (!node) return "";
    return node.path.startsWith("::") ? node.name : node.path;
  }

  function fileSharePercent(file: DiskFile): string {
    if (scanRootWeight <= 0) return "0.0";
    return ((Math.max(file.allocated_bytes, file.size_bytes) / scanRootWeight) * 100).toFixed(1);
  }

  function attributesLabel(attributes: string): string {
    return attributes ? attributes : "—";
  }

  let displayFiles = $derived(
    searchResult ? searchResult.files : sortTopFiles(filteredTopFiles, fileSort)
  );

  let fileSortLabel = $derived(
    fileSort === "size" ? t("size") : fileSort === "modified" ? t("newest") : t("name")
  );

  let isSearchActive = $derived(
    fileQuery.trim().length > 0 ||
      fileExtensionFilter !== null ||
      fileInclude.trim().length > 0 ||
      fileExclude.trim().length > 0 ||
      (fileScope === "folder" && focusNode !== null)
  );

  const DIR_COLORS = ["#33545A", "#3E5364", "#4B465E", "#5A4B43", "#3C5747", "#45566B"];
  const DIR_COLORS_CB = ["#3A5F8A", "#3E7485", "#5A5E9E", "#7C663D", "#3C6E5A", "#536B7C"];
  const EXT_COLORS: Record<string, string> = {
    code: "#3E8C88",
    web: "#6E6AA8",
    image: "#B98545",
    media: "#A85B6B",
    archive: "#7A6345",
    document: "#65758C",
    binary: "#6B7280",
    other: "#4D6470",
  };
  const EXT_COLORS_CB: Record<string, string> = {
    code: "#56B4E9",
    web: "#0072B2",
    image: "#E69F00",
    media: "#CC79A7",
    archive: "#8C6D31",
    document: "#6B7B8C",
    binary: "#009E73",
    other: "#7A8794",
  };

  let activeDirColors = $derived(theme.colorBlind ? DIR_COLORS_CB : DIR_COLORS);
  const CODE_EXTENSIONS = new Set(["rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "c", "cc", "cpp", "h", "hpp", "cs", "php", "rb", "swift", "kt", "kts", "sh", "ps1"]);
  const WEB_EXTENSIONS = new Set(["html", "htm", "css", "scss", "sass", "vue", "svelte", "astro"]);
  const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "bmp", "avif"]);
  const MEDIA_EXTENSIONS = new Set(["mp4", "mkv", "avi", "mov", "mp3", "wav", "flac", "ogg", "webm"]);
  const ARCHIVE_EXTENSIONS = new Set(["zip", "7z", "rar", "tar", "gz", "xz", "bz2", "iso"]);
  const DOCUMENT_EXTENSIONS = new Set(["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "md", "json", "yaml", "yml", "toml", "xml"]);
  const BINARY_EXTENSIONS = new Set(["exe", "dll", "msi", "bin", "sys", "lib", "pdb", "node"]);

  function extensionGroup(extension: string): string {
    const ext = extension.toLowerCase();
    if (CODE_EXTENSIONS.has(ext)) return "code";
    if (WEB_EXTENSIONS.has(ext)) return "web";
    if (IMAGE_EXTENSIONS.has(ext)) return "image";
    if (MEDIA_EXTENSIONS.has(ext)) return "media";
    if (ARCHIVE_EXTENSIONS.has(ext)) return "archive";
    if (DOCUMENT_EXTENSIONS.has(ext)) return "document";
    if (BINARY_EXTENSIONS.has(ext)) return "binary";
    return "other";
  }

  function colorForExtension(extension: string): string {
    const palette = theme.colorBlind ? EXT_COLORS_CB : EXT_COLORS;
    return palette[extensionGroup(extension)] || palette.other;
  }

  function formatBytes(bytes: number): string {
    if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    const index = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
    return `${(bytes / Math.pow(1024, index)).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
  }

  function formatModified(epochMs: number | null): string {
    if (!epochMs) return "—";
    const date = new Date(epochMs);
    if (Number.isNaN(date.getTime())) return "—";
    const pad = (value: number) => String(value).padStart(2, "0");
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
  }

  function sortTopFiles(files: DiskFile[], sort: "size" | "modified" | "name"): DiskFile[] {
    const sorted = [...files];
    if (sort === "name") {
      sorted.sort((left, right) =>
        left.name.localeCompare(right.name, undefined, { sensitivity: "base" })
      );
    } else if (sort === "modified") {
      sorted.sort(
        (left, right) => (right.modified_epoch_ms ?? 0) - (left.modified_epoch_ms ?? 0)
      );
    } else {
      sorted.sort(
        (left, right) =>
          Math.max(right.allocated_bytes, right.size_bytes) -
          Math.max(left.allocated_bytes, left.size_bytes)
      );
    }
    return sorted;
  }

  function setFileSort(sort: "size" | "modified" | "name") {
    fileSort = sort;
    saveDiskPreferences();
    if (isSearchActive) {
      scheduleFileSearch();
    }
  }

  function saveDiskPreferences() {
    try {
      localStorage.setItem(
        DISK_PREFS_KEY,
        JSON.stringify({
          path: pathInput,
          duplicateMode,
          fileSort,
          fileFilterMode,
          fileInclude,
          fileExclude,
          fileLimit,
          fileScope,
          treemapShowNames,
          treemapUseLogical,
          treemapShowFree,
          treemapGroupSmall,
        })
      );
    } catch {}
  }

  function loadDiskPreferences() {
    try {
      const raw = localStorage.getItem(DISK_PREFS_KEY);
      if (!raw) return;
      const prefs = JSON.parse(raw) as Record<string, unknown>;
      if (typeof prefs.path === "string" && prefs.path.trim()) pathInput = prefs.path;
      if (["off", "name_size", "name_size_date"].includes(prefs.duplicateMode as string)) {
        duplicateMode = prefs.duplicateMode as "off" | "name_size" | "name_size_date";
      }
      if (["size", "modified", "name"].includes(prefs.fileSort as string)) {
        fileSort = prefs.fileSort as "size" | "modified" | "name";
      }
      if (prefs.fileFilterMode === "name" || prefs.fileFilterMode === "path") {
        fileFilterMode = prefs.fileFilterMode;
      }
      if (typeof prefs.fileInclude === "string") fileInclude = prefs.fileInclude;
      if (typeof prefs.fileExclude === "string") fileExclude = prefs.fileExclude;
      if (prefs.fileScope === "all" || prefs.fileScope === "folder") {
        fileScope = prefs.fileScope;
      }
      if ([100, 500, 1000, 2000].includes(Number(prefs.fileLimit))) {
        fileLimit = Number(prefs.fileLimit);
      }
      if (typeof prefs.treemapShowNames === "boolean") {
        treemapShowNames = prefs.treemapShowNames;
      }
      if (typeof prefs.treemapUseLogical === "boolean") {
        treemapUseLogical = prefs.treemapUseLogical;
      }
      if (typeof prefs.treemapShowFree === "boolean") {
        treemapShowFree = prefs.treemapShowFree;
      }
      if ([0, 1024, 65536, 1048576].includes(Number(prefs.treemapGroupSmall))) {
        treemapGroupSmall = Number(prefs.treemapGroupSmall);
      }
    } catch {}
  }

  function nodeWeight(node: DiskTreeNode): number {
    return Math.max(node.allocated_bytes, node.size_bytes);
  }

  function treemapWeight(node: DiskTreeNode): number {
    return treemapUseLogical ? node.size_bytes : nodeWeight(node);
  }

  function isPseudoTreemapNode(node: DiskTreeNode): boolean {
    return node.path.startsWith("::free::") || node.path.startsWith("::small::");
  }

  function treemapChildren(node: DiskTreeNode): DiskTreeNode[] {
    let kids = [...node.children];
    if (treemapGroupSmall > 0) {
      const small = kids.filter(
        (child) =>
          !child.is_dir &&
          treemapWeight(child) > 0 &&
          treemapWeight(child) < treemapGroupSmall
      );
      if (small.length >= 2) {
        kids = kids.filter((child) => !small.includes(child));
        kids.push({
          name: t("Small files"),
          path: `::small::${node.path}`,
          is_dir: false,
          size_bytes: small.reduce((sum, child) => sum + child.size_bytes, 0),
          allocated_bytes: small.reduce(
            (sum, child) => sum + child.allocated_bytes,
            0
          ),
          modified_epoch_ms: null,
          attributes: "",
          file_count: small.reduce((sum, child) => sum + child.file_count, 0),
          children: [],
        });
      }
    }
    return kids.filter((child) => treemapWeight(child) > 0);
  }

  function treemapNodes(node: DiskTreeNode): DiskTreeNode[] {
    const kids = treemapChildren(node);
    if (
      treemapShowFree &&
      result &&
      node.path === result.root_path &&
      isDriveRoot(result.root_path)
    ) {
      const free = Math.max(0, result.volume_free_bytes);
      if (free > 0) {
        kids.push({
          name: t("Free space"),
          path: "::free::",
          is_dir: false,
          size_bytes: free,
          allocated_bytes: free,
          modified_epoch_ms: null,
          attributes: "",
          file_count: 0,
          children: [],
        });
      }
    }
    return kids.sort((left, right) => treemapWeight(right) - treemapWeight(left));
  }

  function percentOfFocusedNode(node: DiskTreeNode): string {
    const base = nodeWeight(focusNode ?? result?.root ?? node);
    if (base <= 0) return "0.0";
    return ((nodeWeight(node) / base) * 100).toFixed(1);
  }

  function isDriveRoot(value: string): boolean {
    return /^[a-zA-Z]:[\\/]*$/.test(value.trim());
  }

  function activeSearchPrefix(): string | null {
    if (fileScope !== "folder" || !focusNode) return null;
    // A merged multi-root scan uses a synthetic root that is not a real path.
    if (focusNode.path.startsWith("::")) return null;
    return focusNode.path;
  }

  function clearFileSearch() {
    clearTimeout(searchTimer);
    searchResult = null;
    isSearching = false;
    searchSequence += 1;
  }

  function scheduleFileSearch() {
    clearTimeout(searchTimer);
    if (!isSearchActive) {
      clearFileSearch();
      return;
    }
    searchTimer = setTimeout(runFileSearch, 200);
  }

  async function runFileSearch() {
    if (!result || !isSearchActive) {
      searchResult = null;
      return;
    }

    const sequence = ++searchSequence;
    isSearching = true;
    try {
      const response = await invoke<DiskFileSearchResult>("search_disk_files", {
        path: result.root_path,
        query: fileQuery.trim(),
        extension: fileExtensionFilter,
        prefix: activeSearchPrefix(),
        filterMode: fileFilterMode,
        include: fileInclude.trim() || null,
        exclude: fileExclude.trim() || null,
        sort: fileSort,
        limit: fileLimit,
      });
      if (sequence === searchSequence) {
        searchResult = response;
      }
    } catch {
      if (sequence === searchSequence) {
        searchResult = null;
      }
    } finally {
      if (sequence === searchSequence) {
        isSearching = false;
      }
    }
  }

  function showFilesByExtension(extension: string) {
    fileExtensionFilter = extension;
    fileQuery = "";
    activeView = "files";
    scheduleFileSearch();
  }

  function clearExtensionFilter() {
    fileExtensionFilter = null;
    scheduleFileSearch();
  }

  function setFileScope(scope: "all" | "folder") {
    fileScope = scope;
    saveDiskPreferences();
    scheduleFileSearch();
  }

  function setFileFilterMode(mode: "name" | "path") {
    fileFilterMode = mode;
    saveDiskPreferences();
    if (isSearchActive) {
      scheduleFileSearch();
    }
  }

  function updatePathFilters() {
    saveDiskPreferences();
    scheduleFileSearch();
  }

  function clearPathFilters() {
    fileInclude = "";
    fileExclude = "";
    updatePathFilters();
  }

  async function scanDisk() {
    if (isImporting) return;
    const targets = pathInput
      .split(/[;\n]+/)
      .map((value) => value.trim())
      .filter(Boolean);
    if (targets.length === 0) {
      toast.show(t("Enter a drive or folder path first."), "warning");
      return;
    }
    saveDiskPreferences();

    isScanning = true;
    isCancelling = false;
    progress = null;
    errorMessage = "";
    result = null;
    focusNode = null;
    focusStack = [];
    selectedNode = null;
    fileQuery = "";
    fileExtensionFilter = null;
    fileScope = "all";
    duplicateSelection = {};
    clearFileSearch();

    try {
      const scanResult = targets.length > 1
        ? await invoke<DiskScanResult>("scan_disk_space_multi", {
            paths: targets,
            maxDepth: 4,
            maxChildren: 40,
            duplicateMode,
          })
        : await invoke<DiskScanResult>("scan_disk_space", {
            path: targets[0],
            maxDepth: 4,
            maxChildren: 40,
            duplicateMode,
          });
      lastScanHadDuplicates = scanResult.duplicate_mode !== "off";
      result = scanResult;
      focusStack = [scanResult.root];
      focusNode = scanResult.root;
      activeView = "treemap";
      toast.show(
        t("Scanned {count} files in {seconds}s.", {
          count: scanResult.total_files.toLocaleString(),
          seconds: (scanResult.elapsed_ms / 1000).toFixed(1),
        }),
        "success"
      );
    } catch (e: any) {
      const message = e.toString();
      if (message.toLowerCase().includes("cancelled")) {
        errorMessage = t("Scan cancelled.");
      } else {
        errorMessage = message;
        toast.show(t("Disk scan failed: {message}", { message }), "error");
      }
    } finally {
      isScanning = false;
      isCancelling = false;
    }
  }

  async function cancelScan() {
    isCancelling = true;
    try {
      await invoke("cancel_disk_scan");
    } catch (e) {
      console.error("Cancel failed:", e);
    }
  }

  function openCsvPicker() {
    if (isScanning || isImporting) return;
    csvInputEl?.click();
  }

  async function importCsv(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    if (file.size > 256 * 1024 * 1024) {
      toast.show(t("CSV is larger than 256 MB. Export a narrower subtree."), "warning");
      return;
    }

    isImporting = true;
    errorMessage = "";
    try {
      const csv = await file.text();
      const scanResult = await invoke<DiskScanResult>("import_disk_csv", {
        csv,
        sourceName: file.name,
        duplicateMode,
      });
      lastScanHadDuplicates = scanResult.duplicate_mode !== "off";
      result = scanResult;
      focusStack = [scanResult.root];
      focusNode = scanResult.root;
      selectedNode = null;
      hoverPath = null;
      tooltip = null;
      fileQuery = "";
      fileExtensionFilter = null;
      fileScope = "all";
      duplicateSelection = {};
      clearFileSearch();
      activeView = "treemap";
      pathInput = scanResult.root_path;
      saveDiskPreferences();
      toast.show(
        t("Imported {count} files from {name}. This snapshot is read-only.", {
          count: scanResult.total_files.toLocaleString(),
          name: file.name,
        }),
        "success"
      );
    } catch (e: any) {
      const message = e.toString();
      errorMessage = message;
      toast.show(t("CSV import failed: {message}", { message }), "error");
    } finally {
      isImporting = false;
    }
  }

  async function loadMftDumps() {
    isLoadingMftDumps = true;
    try {
      mftDumps = await invoke<MftDumpInfo[]>("list_mft_dumps");
    } catch (e) {
      console.error("Failed to list MFT dumps:", e);
    } finally {
      isLoadingMftDumps = false;
    }
  }

  async function toggleMftDumps() {
    showMftDumps = !showMftDumps;
    if (showMftDumps) {
      await loadMftDumps();
    }
  }

  function resetScanView() {
    result = null;
    focusNode = null;
    focusStack = [];
    selectedNode = null;
    hoverPath = null;
    tooltip = null;
    fileQuery = "";
    fileExtensionFilter = null;
    fileScope = "all";
    duplicateSelection = {};
    clearFileSearch();
  }

  async function dumpMft() {
    if (isScanning || isImporting) return;
    if (scanTargets.length !== 1) {
      toast.show(t("MFT dumps support one drive or folder at a time."), "warning");
      return;
    }
    if (!isAdmin) {
      toast.show(t("Restart PurgeKit as Administrator to dump the MFT."), "warning");
      return;
    }
    const target = scanTargets[0];
    const confirmed = confirm(
      t("Dump the $MFT for {target}?", { target }) +
      "\n\n" +
      t("This reads the live NTFS Master File Table, which can take a while on large volumes. The dump is stored under {path} and can be loaded later without Administrator.", {
        path: "%APPDATA%\\PurgeKit\\mft_dumps",
      })
    );
    if (!confirmed) return;

    isScanning = true;
    isCancelling = false;
    progress = null;
    errorMessage = "";
    resetScanView();

    try {
      const response = await invoke<MftDumpResponse>("dump_mft_scan", {
        path: target,
        maxDepth: 8,
        maxChildren: 100,
        duplicateMode,
      });
      const scanResult = response.result;
      lastScanHadDuplicates = scanResult.duplicate_mode !== "off";
      result = scanResult;
      focusStack = [scanResult.root];
      focusNode = scanResult.root;
      activeView = "treemap";
      const dumpName = response.dump_path.split(/[\\/]/).pop() || response.dump_path;
      toast.show(
        t("Scanned {count} files and saved {name}.", {
          count: scanResult.total_files.toLocaleString(),
          name: dumpName,
        }),
        "success"
      );
      await loadMftDumps();
    } catch (e: any) {
      const message = e.toString();
      errorMessage = message;
      toast.show(t("MFT dump failed: {message}", { message }), "error");
    } finally {
      isScanning = false;
      isCancelling = false;
    }
  }

  async function loadSavedDump(dump: MftDumpInfo) {
    if (isScanning) return;
    const confirmed = confirm(
      t("Load {name}?", { name: dump.name }) +
      "\n\n" +
      t("This is a read-only snapshot. Quarantine and cleanup are disabled until you rescan the drive.")
    );
    if (!confirmed) return;

    isScanning = true;
    isCancelling = false;
    loadingDumpPath = dump.path;
    errorMessage = "";
    resetScanView();

    try {
      const scanResult = await invoke<DiskScanResult>("load_mft_dump", {
        path: dump.path,
        maxDepth: 8,
        maxChildren: 100,
        duplicateMode,
      });
      lastScanHadDuplicates = scanResult.duplicate_mode !== "off";
      result = scanResult;
      focusStack = [scanResult.root];
      focusNode = scanResult.root;
      activeView = "treemap";
      pathInput = scanResult.root_path;
      showMftDumps = false;
      saveDiskPreferences();
      toast.show(
        t("Loaded {count} files from {name}. This snapshot is read-only.", {
          count: scanResult.total_files.toLocaleString(),
          name: dump.name,
        }),
        "success"
      );
    } catch (e: any) {
      const message = e.toString();
      errorMessage = message;
      toast.show(t("Could not load MFT dump: {message}", { message }), "error");
    } finally {
      isScanning = false;
      isCancelling = false;
      loadingDumpPath = null;
    }
  }

  async function openMftDumpFolder() {
    try {
      const directory = await invoke<string>("get_mft_dump_dir");
      await openPath(directory);
    } catch (e: any) {
      toast.show(t("Could not open the dump folder: {message}", { message: e.toString() }), "error");
    }
  }

  function binaryTreemap(
    items: DiskTreeNode[],
    x: number,
    y: number,
    w: number,
    h: number,
    output: LayoutRect[] = []
  ): LayoutRect[] {
    if (items.length === 0 || w <= 1 || h <= 1) return output;
    if (items.length === 1) {
      output.push({ node: items[0], x, y, w, h, depth: 0 });
      return output;
    }

    const total = items.reduce((sum, item) => sum + treemapWeight(item), 0);
    if (total <= 0) return output;

    let accumulated = 0;
    let splitIndex = 1;
    for (let index = 0; index < items.length - 1; index++) {
      accumulated += treemapWeight(items[index]);
      if (accumulated >= total / 2) {
        splitIndex = index + 1;
        break;
      }
    }

    const first = items.slice(0, splitIndex);
    const second = items.slice(splitIndex);
    const firstTotal = first.reduce((sum, item) => sum + treemapWeight(item), 0);
    const ratio = Math.max(0.02, Math.min(0.98, firstTotal / total));

    if (w >= h) {
      const firstWidth = w * ratio;
      binaryTreemap(first, x, y, firstWidth, h, output);
      binaryTreemap(second, x + firstWidth, y, w - firstWidth, h, output);
    } else {
      const firstHeight = h * ratio;
      binaryTreemap(first, x, y, w, firstHeight, output);
      binaryTreemap(second, x, y + firstHeight, w, h - firstHeight, output);
    }

    return output;
  }

  const NESTED_MAX_DEPTH = 4;
  const NESTED_MIN_AREA = 1500;

  function nestedTreemap(
    items: DiskTreeNode[],
    x: number,
    y: number,
    w: number,
    h: number,
    depth: number,
    output: LayoutRect[] = []
  ): LayoutRect[] {
    if (items.length === 0 || w <= 2 || h <= 2) return output;

    const levelRects = binaryTreemap(items, x, y, w, h);
    for (const rect of levelRects) {
      output.push({ ...rect, depth });

      const child = rect.node;
      if (
        depth >= NESTED_MAX_DEPTH ||
        !child.is_dir ||
        child.children.length === 0 ||
        rect.w * rect.h < NESTED_MIN_AREA
      ) {
        continue;
      }

      const kids = treemapChildren(child).sort(
        (left, right) => treemapWeight(right) - treemapWeight(left)
      );
      if (kids.length === 0) continue;

      const hasLabel = depth === 0
        ? rect.w > 70 && rect.h > 48
        : rect.w > 54 && rect.h > 30;
      const header = hasLabel ? (depth === 0 ? 36 : 16) : 4;
      const innerW = rect.w - 4;
      const innerH = rect.h - header - 2;
      if (innerW <= 6 || innerH <= 6) continue;

      nestedTreemap(kids, rect.x + 2, rect.y + header, innerW, innerH, depth + 1, output);
    }

    return output;
  }

  function colorForNode(node: DiskTreeNode, index: number): string {
    let hash = 0;
    for (const character of node.name) {
      hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
    }
    if (node.is_dir) {
      return activeDirColors[(hash + index) % activeDirColors.length];
    }
    const extension = node.name.includes(".")
      ? node.name.split(".").pop() || ""
      : "";
    return colorForExtension(extension);
  }

  function drawTreemap() {
    const canvas = canvasEl;
    const node = focusNode;
    if (!canvas || !node || activeView !== "treemap") return;

    const parent = canvas.parentElement;
    if (!parent) return;

    const width = Math.max(320, parent.clientWidth);
    const height = Math.max(280, parent.clientHeight);
    const ratio = window.devicePixelRatio || 1;
    canvas.width = Math.round(width * ratio);
    canvas.height = Math.round(height * ratio);
    canvas.style.width = "100%";
    canvas.style.height = `${height}px`;

    const context = canvas.getContext("2d");
    if (!context) return;
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, width, height);
    context.fillStyle = "#101318";
    context.fillRect(0, 0, width, height);

    const children = treemapNodes(node);

    if (children.length === 0) {
      context.fillStyle = "#8a94a3";
      context.font = "13px system-ui, Segoe UI, sans-serif";
      context.textAlign = "center";
      context.fillText(t("No deeper detail for this folder."), width / 2, height / 2 - 8);
      context.font = "11px ui-monospace, Consolas, monospace";
      context.fillText(node.path, width / 2, height / 2 + 14, width - 32);
      layoutRects = [];
      return;
    }

    layoutRects = nestedTreemap(children, 2, 2, width - 4, height - 4, 0);
    layoutRects.forEach((rect, index) => {
      const highlighted =
        rect.node.path === hoverPath || rect.node.path === selectedNode?.path;
      context.fillStyle =
        rect.node.path === "::free::"
          ? "#2E3946"
          : colorForNode(rect.node, index);
      context.fillRect(rect.x, rect.y, rect.w, rect.h);
      if (rect.depth > 0) {
        // Deeper levels sit visually inside their folder instead of competing
        // with it.
        context.fillStyle = "rgba(9, 11, 15, 0.12)";
        context.fillRect(rect.x, rect.y, rect.w, rect.h);
      }

      context.strokeStyle = highlighted ? "#F4F5F7" : "rgba(9, 11, 15, 0.85)";
      context.lineWidth = highlighted ? 2 : 1;
      context.strokeRect(rect.x + 0.5, rect.y + 0.5, Math.max(0, rect.w - 1), Math.max(0, rect.h - 1));

      const hasLabel =
        treemapShowNames &&
        (rect.depth === 0
          ? rect.w > 70 && rect.h > 48
          : rect.w > 54 && rect.h > 30);
      if (hasLabel) {
        context.save();
        context.beginPath();
        context.rect(rect.x + 3, rect.y + 3, rect.w - 6, rect.h - 6);
        context.clip();
        context.fillStyle = "#f4f6f8";
        context.font = rect.depth === 0
          ? "11px ui-monospace, Consolas, monospace"
          : "10px ui-monospace, Consolas, monospace";
        context.textAlign = "left";
        context.fillText(rect.node.name, rect.x + 7, rect.y + (rect.depth === 0 ? 18 : 13), rect.w - 14);
        if (rect.depth === 0) {
          context.fillStyle = "rgba(244, 246, 248, 0.7)";
          context.font = "10px ui-monospace, Consolas, monospace";
          context.fillText(formatBytes(treemapWeight(rect.node)), rect.x + 7, rect.y + 33, rect.w - 14);
        }
        context.restore();
      }
    });
  }

  function handleCanvasClick(event: MouseEvent) {
    const canvas = canvasEl;
    if (!canvas) return;
    const bounds = canvas.getBoundingClientRect();
    const x = event.clientX - bounds.left;
    const y = event.clientY - bounds.top;
    const hit = rectAt(x, y);
    if (!hit || isPseudoTreemapNode(hit.node)) return;

    selectedNode = hit.node;
    if (hit.node.is_dir && hit.node.children.length > 0) {
      const path = result ? pathToNode(result.root, hit.node.path) : [hit.node];
      focusStack = path.length > 0 ? path : [hit.node];
      focusNode = hit.node;
    }
  }

  function pathToNode(rootNode: DiskTreeNode, targetPath: string): DiskTreeNode[] {
    const stack: DiskTreeNode[] = [];
    const walk = (current: DiskTreeNode): boolean => {
      stack.push(current);
      if (current.path === targetPath) return true;
      for (const child of current.children) {
        if (walk(child)) return true;
      }
      stack.pop();
      return false;
    };
    return walk(rootNode) ? stack : [];
  }

  function rectAt(x: number, y: number): LayoutRect | undefined {
    // Rectangles are emitted parent-first; reverse order makes the deepest
    // nested rectangle win a hit.
    for (let index = layoutRects.length - 1; index >= 0; index--) {
      const rect = layoutRects[index];
      if (x >= rect.x && x <= rect.x + rect.w && y >= rect.y && y <= rect.y + rect.h) {
        return rect;
      }
    }
    return undefined;
  }

  function handleCanvasMove(event: MouseEvent) {
    const canvas = canvasEl;
    if (!canvas) return;
    const bounds = canvas.getBoundingClientRect();
    const x = event.clientX - bounds.left;
    const y = event.clientY - bounds.top;
    const hit = rectAt(x, y);
    hoverPath = hit?.node.path || null;
    if (hit) {
      // Keep the tooltip inside the canvas container instead of clipping it
      // against the near edge.
      const tooltipWidth = 332;
      const tooltipHeight = 86;
      const left = x + tooltipWidth + 12 > bounds.width
        ? Math.max(4, x - tooltipWidth - 12)
        : x + 12;
      const top = y + tooltipHeight + 12 > bounds.height
        ? Math.max(4, y - tooltipHeight - 12)
        : y + 12;
      tooltip = { x: left, y: top, node: hit.node };
    } else {
      tooltip = null;
    }
    canvas.style.cursor = hit ? (hit.node.is_dir ? "zoom-in" : "pointer") : "crosshair";
  }

  function handleCanvasLeave() {
    hoverPath = null;
    tooltip = null;
  }

  function exportTreemapPng() {
    if (!canvasEl) return;
    const link = document.createElement("a");
    link.href = canvasEl.toDataURL("image/png");
    link.download = `purgekit_treemap_${new Date().toISOString().slice(0, 10)}.png`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  }

  function exportTopFilesCsv() {
    if (!result || displayFiles.length === 0) {
      toast.show(t("No files to export for this filter."), "warning");
      return;
    }

    const escapeCsv = (value: string) => `"${value.replace(/"/g, '""')}"`;
    const duplicateColumns = result.duplicate_mode !== "off";
    const rows = [
      ["Name", "Path", "Extension", "Modified", "Attributes", "SharePercent", "LogicalBytes", "AllocatedBytes", ...(duplicateColumns ? ["DupCount", "DupWasteBytes"] : [])],
      ...displayFiles.map((file) => [
        file.name,
        file.path,
        file.extension,
        file.modified_epoch_ms ? formatModified(file.modified_epoch_ms) : "",
        file.attributes,
        fileSharePercent(file),
        String(file.size_bytes),
        String(file.allocated_bytes),
        ...(duplicateColumns ? [String(duplicateStats.get(file.path)?.count ?? 0), String(duplicateStats.get(file.path)?.waste ?? 0)] : []),
      ]),
    ];
    const csv = "\uFEFF" + rows.map((row) => row.map(escapeCsv).join(",")).join("\n");
    const blob = new Blob([csv], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `purgekit_disk_${new Date().toISOString().slice(0, 10)}.csv`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
  }

  async function exportAllFilesCsv() {
    if (!result) return;
    try {
      const exported = await invoke<DiskCsvExport>("export_disk_files_csv", {
        path: result.root_path,
        query: fileQuery.trim(),
        extension: fileExtensionFilter,
        prefix: activeSearchPrefix(),
        filterMode: fileFilterMode,
        include: fileInclude.trim() || null,
        exclude: fileExclude.trim() || null,
        sort: fileSort,
        limit: 100000,
      });
      const blob = new Blob([exported.csv], { type: "text/csv;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `purgekit_disk_all_${new Date().toISOString().slice(0, 10)}.csv`;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      URL.revokeObjectURL(url);
      toast.show(
        exported.truncated
          ? t("Exported {count} of {total} files (100k cap).", {
              count: exported.exported.toLocaleString(),
              total: exported.total.toLocaleString(),
            })
          : t("Exported {count} files.", { count: exported.exported.toLocaleString() }),
        exported.truncated ? "warning" : "success"
      );
    } catch (e: any) {
      toast.show(t("Export failed: {message}", { message: e.toString() }), "error");
    }
  }

  function escapeHtml(value: string): string {
    return value
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function collectTopFolders(rootNode: DiskTreeNode, limit: number): DiskTreeNode[] {
    const folders: DiskTreeNode[] = [];
    const walk = (node: DiskTreeNode, depth: number) => {
      if (!node.is_dir || depth > 8) return;
      if (depth > 0) folders.push(node);
      for (const child of node.children) {
        if (child.is_dir) walk(child, depth + 1);
      }
    };
    walk(rootNode, 0);
    return folders
      .sort((left, right) => nodeWeight(right) - nodeWeight(left))
      .slice(0, limit);
  }

  function exportDiskHtml() {
    if (!result) return;
    const fileRows = result.top_files
      .map(
        (file) => `<tr>
          <td>${escapeHtml(file.name)}</td>
          <td class="path">${escapeHtml(file.path)}</td>
          <td class="num">${escapeHtml(formatBytes(Math.max(file.allocated_bytes, file.size_bytes)))}</td>
          <td class="num">${escapeHtml(formatBytes(file.size_bytes))}</td>
          <td class="num">${escapeHtml(fileSharePercent(file))}%</td>
          <td class="num">${escapeHtml(file.modified_epoch_ms ? formatModified(file.modified_epoch_ms) : "—")}</td>
        </tr>`
      )
      .join("");
    const folderRows = collectTopFolders(result.root, 100)
      .map(
        (folder) => `<tr>
          <td>${escapeHtml(folder.name)}</td>
          <td class="path">${escapeHtml(folder.path)}</td>
          <td class="num">${escapeHtml(formatBytes(nodeWeight(folder)))}</td>
          <td class="num">${folder.file_count.toLocaleString()}</td>
        </tr>`
      )
      .join("");
    const typeRows = result.file_types
      .map(
        (type) => `<tr>
          <td>${escapeHtml(type.extension || t("(none)"))}</td>
          <td class="num">${escapeHtml(formatBytes(type.size_bytes))}</td>
          <td class="num">${type.file_count.toLocaleString()}</td>
        </tr>`
      )
      .join("");
    const duplicateRows = result.duplicate_groups
      .slice(0, 50)
      .map(
        (group) => `<tr>
          <td>${escapeHtml(group.name)}</td>
          <td class="num">${group.file_count}</td>
          <td class="num">${escapeHtml(formatBytes(group.size_bytes))}</td>
          <td class="num">${escapeHtml(formatBytes(group.size_bytes * (group.file_count - 1)))}</td>
        </tr>`
      )
      .join("");

    const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>PurgeKit disk report — ${escapeHtml(result.root_path)}</title>
<style>
:root{color-scheme:light}body{margin:0;padding:32px;background:#f4f1eb;color:#171a1f;font:14px/1.45 system-ui,Segoe UI,sans-serif}
main{max-width:1100px;margin:auto}h1{font-size:24px;margin:0 0 4px}h2{font-size:15px;margin:28px 0 8px}p{color:#5c646e}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:8px;margin:18px 0}
.card{background:#fff;border:1px solid #d8d2c8;border-radius:8px;padding:12px}.label{font:10px ui-monospace,monospace;text-transform:uppercase;letter-spacing:.08em;color:#707781}.value{font:600 19px ui-monospace,monospace;margin-top:4px;color:#c94e1b}
table{width:100%;border-collapse:collapse;background:#fff;border:1px solid #d8d2c8;border-radius:8px;overflow:hidden}
th,td{padding:7px 10px;border-bottom:1px solid #e8e3da;text-align:left;vertical-align:top}th{background:#ede9e2;font:10px ui-monospace,monospace;text-transform:uppercase;letter-spacing:.06em;color:#4b525c}
tr:last-child td{border-bottom:0}.num{font-family:ui-monospace,monospace;white-space:nowrap}.path{font-family:ui-monospace,monospace;font-size:11px;color:#5c646e;word-break:break-all}
small{color:#707781}footer{margin-top:28px;font-size:11px;color:#707781}
</style>
</head>
<body><main>
<h1>PurgeKit disk report</h1>
<p><strong>${escapeHtml(result.root_path)}</strong> · ${escapeHtml(result.engine.toUpperCase())} · generated ${escapeHtml(new Date().toLocaleString())}</p>
<div class="grid">
<div class="card"><div class="label">Allocated</div><div class="value">${escapeHtml(formatBytes(result.total_allocated))}</div></div>
<div class="card"><div class="label">Logical</div><div class="value">${escapeHtml(formatBytes(result.total_size))}</div></div>
<div class="card"><div class="label">Files</div><div class="value">${result.total_files.toLocaleString()}</div></div>
<div class="card"><div class="label">Folders</div><div class="value">${result.total_dirs.toLocaleString()}</div></div>
<div class="card"><div class="label">Volume free</div><div class="value">${escapeHtml(formatBytes(result.volume_free_bytes))}</div></div>
<div class="card"><div class="label">Errors</div><div class="value">${result.errors.toLocaleString()}</div></div>
</div>
${result.duplicate_mode !== "off" ? `<p><small>Duplicate grouping: ${escapeHtml(result.duplicate_mode)} (name/size/date, contents not compared).</small></p>` : ""}
<h2>Largest files</h2><table><thead><tr><th>File</th><th>Path</th><th>Size</th><th>Logical</th><th>Share</th><th>Modified</th></tr></thead><tbody>${fileRows || "<tr><td colspan=6>No files</td></tr>"}</tbody></table>
<h2>Largest folders</h2><table><thead><tr><th>Folder</th><th>Path</th><th>Size</th><th>Files</th></tr></thead><tbody>${folderRows || "<tr><td colspan=4>No folders</td></tr>"}</tbody></table>
<h2>File types</h2><table><thead><tr><th>Extension</th><th>Size</th><th>Files</th></tr></thead><tbody>${typeRows || "<tr><td colspan=3>No types</td></tr>"}</tbody></table>
${duplicateRows ? `<h2>Duplicate groups (top 50)</h2><table><thead><tr><th>Name</th><th>Copies</th><th>Size each</th><th>Potential waste</th></tr></thead><tbody>${duplicateRows}</tbody></table>` : ""}
<footer>PurgeKit · report contains the top 100 files/folders from this scan. Snapshot values are read-only.</footer>
</main></body></html>`;

    const blob = new Blob([html], { type: "text/html;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `purgekit_disk_report_${new Date().toISOString().slice(0, 10)}.html`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
    toast.show(t("Exported disk report as HTML."), "success");
  }

  function duplicatesSelectedInGroup(group: DuplicateGroup): number {
    return group.files.filter((file) => duplicateSelection[file.path]).length;
  }

  function toggleDuplicateFile(group: DuplicateGroup, file: DiskFile) {
    if (!duplicateSelection[file.path] && duplicatesSelectedInGroup(group) >= group.file_count - 1) {
      toast.show(t("Keep at least one copy in each duplicate group."), "warning");
      return;
    }
    duplicateSelection[file.path] = !duplicateSelection[file.path];
  }

  function selectAllButFirst(group: DuplicateGroup) {
    for (let index = 1; index < group.files.length; index++) {
      duplicateSelection[group.files[index].path] = true;
    }
  }

  function clearDuplicateSelection() {
    duplicateSelection = {};
  }

  async function quarantineSelectedDuplicates() {
    if (!result || selectedDuplicateCount === 0 || isQuarantining) return;
    if (isSnapshotView) {
      toast.show(t("Snapshots are read-only. Rescan the drive to clean files."), "warning");
      return;
    }

    const selected = result.duplicate_groups.flatMap((group) =>
      group.files.filter((file) => duplicateSelection[file.path])
    );
    const bytes = selected.reduce(
      (sum, file) => sum + Math.max(file.allocated_bytes, file.size_bytes),
      0
    );
    const confirmed = confirm(
      t("Quarantine {count} duplicate copies ({size} reclaimable)?", {
        count: selected.length,
        size: formatBytes(bytes),
      }) +
      "\n\n" +
      t("Copies are moved to PurgeKit quarantine when backup is enabled; otherwise they are deleted permanently. At least one copy of each group stays in place. The scan will refresh afterwards.")
    );
    if (!confirmed) return;

    isQuarantining = true;
    let failed = 0;
    for (const file of selected) {
      try {
        const response = await invoke<{ success: number; fail: number; errors?: string[] }>(
          "force_remove_app",
          {
            installLocation: file.path,
            registryPath: null,
            hive: null,
          }
        );
        if (response.fail > 0) failed += 1;
      } catch {
        failed += 1;
      }
    }

    duplicateSelection = {};
    isQuarantining = false;
    toast.show(
      failed > 0
        ? t("Quarantined {ok} copies; {failed} failed. Refreshing scan…", {
            ok: selected.length - failed,
            failed,
          })
        : t("Quarantined {count} copies. Refreshing scan…", { count: selected.length }),
      failed > 0 ? "warning" : "success"
    );

    await scanDisk();
    if (duplicateMode !== "off" && result) activeView = "duplicates";
  }

  async function quarantineSelected() {
    const node = selectedNode;
    if (!node || !result) return;
    if (isSnapshotView) {
      toast.show(t("Snapshots are read-only. Rescan the drive to clean files."), "warning");
      return;
    }
    const protectedRoots = result.protected_roots?.length
      ? result.protected_roots
      : [result.root_path];
    if (protectedRoots.some((root) => root.toLowerCase() === node.path.toLowerCase())) {
      toast.show(t("Select a child folder or file first; scan roots are protected."), "warning");
      return;
    }

    const confirmed = confirm(
      t('Quarantine "{name}"?', { name: node.name }) +
      "\n\n" +
      node.path +
      "\n\n" +
      t("The item is moved to PurgeKit quarantine and can be restored from Settings. The scan result will be cleared.")
    );
    if (!confirmed) return;

    isQuarantining = true;
    try {
      const response = await invoke<{ success: number; fail: number; errors?: string[] }>(
        "force_remove_app",
        {
          installLocation: node.path,
          registryPath: null,
          hive: null,
        }
      );

      if (response.fail > 0) {
        toast.show(
          t("Quarantine failed: {message}", {
            message: response.errors?.[0] || t("see logs"),
          }),
          "error"
        );
        return;
      }

      toast.show(t("Quarantined {name}.", { name: node.name }), "success");
      result = null;
      focusNode = null;
      focusStack = [];
      selectedNode = null;
      hoverPath = null;
      tooltip = null;
      layoutRects = [];
      duplicateSelection = {};
    } catch (e: any) {
      toast.show(t("Quarantine failed: {message}", { message: e.toString() }), "error");
    } finally {
      isQuarantining = false;
    }
  }

  function goToFocus(index: number) {
    focusStack = focusStack.slice(0, index + 1);
    focusNode = focusStack[focusStack.length - 1] || null;
    selectedNode = null;
  }

  async function openSelected() {
    const node = selectedNode || focusNode;
    if (!node) return;
    if (node.path.startsWith("::")) {
      toast.show(t("Select one of the scanned roots to open it in Explorer."), "warning");
      return;
    }
    const folder = node.is_dir
      ? node.path
      : node.path.slice(0, Math.max(0, node.path.lastIndexOf("\\")));
    try {
      await openPath(folder);
    } catch (e: any) {
      toast.show(t("Could not open folder: {message}", { message: e.toString() }), "error");
    }
  }

  async function openFileFolder(file: DiskFile) {
    const folder = file.path.slice(0, Math.max(0, file.path.lastIndexOf("\\")));
    try {
      await openPath(folder);
    } catch (e: any) {
      toast.show(t("Could not open folder: {message}", { message: e.toString() }), "error");
    }
  }

  $effect(() => {
    // Track the values that affect the canvas, then redraw after layout.
    focusNode;
    activeView;
    canvasEl;
    hoverPath;
    selectedNode;
    theme.colorBlind;
    treemapShowNames;
    treemapUseLogical;
    treemapShowFree;
    treemapGroupSmall;
    if (canvasEl && activeView === "treemap") {
      requestAnimationFrame(drawTreemap);
    }
  });

  $effect(() => {
    if (activeView === "files" && result && isSearchActive) {
      scheduleFileSearch();
    }
  });

  $effect(() => {
    if (!canvasEl) return;
    const parent = canvasEl.parentElement;
    if (!parent) return;

    const observer = new ResizeObserver(() => {
      if (activeView === "treemap") drawTreemap();
    });
    observer.observe(parent);
    return () => observer.disconnect();
  });

  onMount(() => {
    loadDiskPreferences();
    invoke<DriveInfo[]>("list_disk_drives")
      .then((list) => { drives = list; })
      .catch(() => {});
    invoke<boolean>("check_is_admin")
      .then((value) => { isAdmin = value; })
      .catch(() => {});
    loadMftDumps();

    const unlisten = listen<DiskProgress>("disk-scan-progress", (event) => {
      progress = event.payload;
    });

    const onResize = () => {
      if (activeView === "treemap") drawTreemap();
    };
    window.addEventListener("resize", onResize);

    return () => {
      clearTimeout(searchTimer);
      unlisten.then((cleanup) => cleanup());
      window.removeEventListener("resize", onResize);
    };
  });
</script>

<div class="flex-1 flex flex-col h-full bg-app-bg text-text-primary">
  <header class="p-6 border-b border-border-default flex flex-col gap-4 bg-sidebar-bg">
    <div class="flex items-center justify-between gap-4">
      <div>
        <h2 class="text-2xl font-bold font-sans text-text-primary flex items-center gap-2">
          <HardDrive class="w-6 h-6 text-accent" />
          {t("Disk Space")}
        </h2>
        <p class="text-xs text-text-muted mt-1">
          {t("Fast folder scan with allocated size, tree map, largest files and file types.")}
        </p>
      </div>
      <div class="flex items-center gap-2">
        <input
          bind:this={csvInputEl}
          type="file"
          accept=".csv,.tsv,text/csv,text/tab-separated-values"
          onchange={importCsv}
          class="hidden"
          tabindex="-1"
        />
        {#if isScanning}
          <button
            onclick={cancelScan}
            disabled={isCancelling}
            class="px-3 py-2 rounded-lg bg-danger/10 hover:bg-danger-strong text-danger hover:text-white border border-danger/20 text-xs font-semibold transition-all disabled:opacity-40"
          >
            <Square class="w-3.5 h-3.5 inline mr-1" />
            {isCancelling ? t("Stopping…") : t("Cancel")}
          </button>
        {:else}
          <button
            onclick={openCsvPicker}
            disabled={isImporting}
            title={t("Load a PurgeKit or WizTree CSV export as a read-only tree")}
            class="flex items-center gap-2 px-3 py-2 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary text-xs font-semibold transition-colors disabled:opacity-40"
          >
            <Upload class="w-3.5 h-3.5" />
            {isImporting ? t("Importing…") : t("Import CSV")}
          </button>
          <button
            onclick={scanDisk}
            disabled={isImporting}
            class="flex items-center gap-2 px-4 py-2 rounded-lg bg-accent-strong hover:bg-accent-strong/90 text-white text-xs font-semibold transition-all active:scale-95 disabled:opacity-40"
          >
            <Play class="w-3.5 h-3.5" />
            {scanTargets.length > 1
              ? t("Scan {count} paths", { count: scanTargets.length })
              : t("Scan")}
          </button>
        {/if}
      </div>
    </div>

    <div class="flex flex-col gap-2">
      <div class="relative">
        <input
          bind:value={pathInput}
          onkeydown={(event) => event.key === "Enter" && !isScanning && scanDisk()}
          placeholder={t("C:\\ or D:\\Code; E:\\Games")}
          aria-label={t("Drives or folders to scan, separated by semicolons")}
          class="w-full px-3 py-2 rounded-lg bg-surface-bg border border-border-default focus:border-accent/50 focus:ring-1 focus:ring-accent/30 outline-none text-sm font-mono text-text-primary placeholder:text-text-muted"
        />
      </div>
      <div class="flex flex-wrap items-center gap-2">
        {#each drives as drive}
          <button
            onclick={() => { pathInput = `${drive.letter}:\\`; scanDisk(); }}
            disabled={isScanning || isImporting}
            title={t("{label} · {free} free of {total}", {
              label: drive.label || t("Local Disk"),
              free: formatBytes(drive.free_bytes),
              total: formatBytes(drive.total_bytes),
            })}
            aria-label={t("Scan drive {letter}:", { letter: drive.letter })}
            class="flex items-center gap-1.5 px-2.5 py-2 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-[11px] font-semibold text-text-secondary disabled:opacity-40 transition-colors"
          >
            <HardDrive class="w-3.5 h-3.5 text-text-muted" aria-hidden="true" />
            <span class="font-mono text-text-primary">{drive.letter}:</span>
            <span class="font-mono text-text-muted">{t("{size} free", { size: formatBytes(drive.free_bytes) })}</span>
          </button>
        {:else}
          <button
            onclick={() => { pathInput = "C:\\"; scanDisk(); }}
            disabled={isScanning || isImporting}
            class="px-3 py-2 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-xs font-semibold text-text-secondary disabled:opacity-40"
          >
            C:\
          </button>
        {/each}
        <label class="flex items-center gap-2 px-3 py-2 rounded-lg bg-surface-bg border border-border-default text-xs font-semibold text-text-secondary">
          {t("Duplicates")}
          <select
            bind:value={duplicateMode}
            onchange={saveDiskPreferences}
            disabled={isScanning || isImporting}
            aria-label={t("Duplicate comparison mode")}
            class="bg-transparent text-[11px] font-mono text-text-primary outline-none cursor-pointer disabled:opacity-40"
          >
            <option value="off">{t("Off")}</option>
            <option value="name_size">{t("Name + size")}</option>
            <option value="name_size_date">{t("Name + size + date")}</option>
          </select>
        </label>
        <div class="flex items-center gap-2 sm:ml-auto">
          <button
            onclick={dumpMft}
            disabled={isScanning || isImporting || !isAdmin}
            title={isAdmin
              ? t("Dump the live NTFS Master File Table for this target")
              : t("Restart PurgeKit as Administrator to dump the MFT")}
            class="flex items-center gap-1.5 px-2.5 py-2 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-[11px] font-semibold text-text-secondary disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
          >
            <Save class="w-3.5 h-3.5" />
            {t("Dump MFT")}
          </button>
          <button
            onclick={toggleMftDumps}
            disabled={isScanning || isImporting}
            class="flex items-center gap-1.5 px-2.5 py-2 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-[11px] font-semibold text-text-secondary disabled:opacity-40 transition-colors"
          >
            <History class="w-3.5 h-3.5" />
            {t("Load dump")}
            {#if mftDumps.length > 0}
              <span class="font-mono text-text-muted">({mftDumps.length})</span>
            {/if}
          </button>
        </div>
      </div>
    </div>

    {#if showMftDumps}
      <div class="p-3 rounded-lg border border-border-default bg-surface-bg/40 space-y-2">
        <div class="flex items-center justify-between gap-2">
          <span class="text-[11px] font-mono uppercase tracking-wider text-text-muted">{t("Saved MFT dumps")}</span>
          <div class="flex items-center gap-1.5">
            <button
              onclick={loadMftDumps}
              disabled={isLoadingMftDumps}
              class="p-1.5 rounded-md bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-muted hover:text-text-primary transition-colors disabled:opacity-40"
              title={t("Refresh dump list")}
              aria-label={t("Refresh MFT dump list")}
            >
              <History class="w-3.5 h-3.5 {isLoadingMftDumps ? 'animate-pulse text-accent' : ''}" />
            </button>
            <button
              onclick={openMftDumpFolder}
              class="px-2 py-1.5 rounded-md bg-surface-bg border border-border-default hover:bg-elevated-bg text-[10px] font-semibold text-text-muted hover:text-text-primary transition-colors"
            >
              {t("Open folder")}
            </button>
            <button
              onclick={() => showMftDumps = false}
              class="p-1.5 rounded-md text-text-muted hover:text-text-primary transition-colors"
              title={t("Hide dump list")}
              aria-label={t("Hide MFT dump list")}
            >
              <X class="w-3.5 h-3.5" />
            </button>
          </div>
        </div>
        {#if mftDumps.length === 0}
          <p class="text-[11px] text-text-muted py-1">
            {t("No dumps yet. Run as Administrator and select a single drive or folder, then choose Dump MFT.")}
          </p>
        {:else}
          <div class="space-y-1 max-h-48 overflow-y-auto">
            {#each mftDumps as dump}
              <div class="flex items-center justify-between gap-3 px-2.5 py-1.5 rounded-md bg-app-bg border border-border-subtle">
                <div class="flex flex-col min-w-0">
                  <span class="text-[11px] font-mono text-text-primary truncate" title={dump.path}>{dump.name}</span>
                  <span class="text-[10px] text-text-muted font-mono">
                    {formatBytes(dump.size_bytes)} · {dump.modified_epoch_ms ? formatModified(dump.modified_epoch_ms) : t("unknown date")}
                  </span>
                </div>
                <button
                  onclick={() => loadSavedDump(dump)}
                  disabled={isScanning || loadingDumpPath === dump.path}
                  class="px-2.5 py-1.5 rounded-md bg-accent/10 hover:bg-accent-strong hover:text-white border border-accent/20 text-accent text-[10px] font-semibold transition-colors disabled:opacity-40"
                >
                  {loadingDumpPath === dump.path ? t("Loading…") : t("Load")}
                </button>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

    {#if isScanning}
      <div class="p-3 rounded-lg bg-accent/5 border border-accent/15 text-xs">
        <div class="flex items-center justify-between gap-3">
          <span class="text-text-secondary">{t("Scanning…")}</span>
          <span class="font-mono text-accent">
            {progress
              ? t("{files} files · {dirs} dirs · {size}", {
                  files: progress.files.toLocaleString(),
                  dirs: progress.dirs.toLocaleString(),
                  size: formatBytes(progress.bytes),
                })
              : t("starting")}
          </span>
        </div>
        <div class="h-1 bg-border-default rounded-full overflow-hidden mt-2">
          <div class="h-full w-1/3 bg-accent animate-pulse rounded-full"></div>
        </div>
        {#if progress}
          <p class="text-[10px] text-text-muted font-mono truncate mt-1">{progress.current_path}</p>
        {/if}
      </div>
    {/if}

    {#if errorMessage}
      <div class="p-3 rounded-lg bg-danger/5 border border-danger/20 text-xs text-danger">
        {errorMessage}
      </div>
    {/if}
  </header>

  <div class="flex-1 overflow-y-auto p-6 space-y-4">
    {#if result}
      <div class="instrument-panel grid grid-cols-2 lg:grid-cols-5 divide-x divide-y lg:divide-y-0 divide-border-default overflow-hidden">
        <div class="px-4 py-3.5">
          <span class="readout-label">{t("Allocated")}</span>
          <p class="readout text-2xl font-semibold text-data-cyan mt-1">{formatBytes(result.total_allocated)}</p>
        </div>
        <div class="px-4 py-3.5">
          <span class="readout-label">{t("Logical")}</span>
          <p class="readout text-2xl font-semibold text-text-primary mt-1">{formatBytes(result.total_size)}</p>
        </div>
        <div class="px-4 py-3.5">
          <span class="readout-label">{t("Files / folders")}</span>
          <p class="readout text-2xl font-semibold text-text-primary mt-1">
            {result.total_files.toLocaleString()}
            <span class="text-base text-text-muted"> / {result.total_dirs.toLocaleString()}</span>
          </p>
        </div>
        <div class="px-4 py-3.5">
          <span class="readout-label">
            {result.engine === "csv" ? t("Import time") : result.engine === "mft-dump" ? t("Load time") : t("Scan time")}
          </span>
          <p class="readout text-2xl font-semibold text-text-primary mt-1">
            {(result.elapsed_ms / 1000).toFixed(2)}<span class="text-base text-text-muted">s</span>
          </p>
          {#if result.errors > 0}
            <p class="text-[11px] text-warning mt-1">{t("{count} entries skipped", { count: result.errors })}</p>
          {/if}
        </div>
        <div class="px-4 py-3.5">
          <span class="readout-label">{t("Volume free")}</span>
          <p class="readout text-2xl font-semibold text-data-green mt-1">{formatBytes(result.volume_free_bytes)}</p>
          <p class="text-[11px] text-text-muted mt-1">{t("of {size}", { size: formatBytes(result.volume_total_bytes) })}</p>
        </div>
      </div>

      <div class="flex flex-wrap items-center gap-2 text-[11px]">
        <span class="px-2 py-1 rounded border font-mono
          {result.engine === 'mft'
            ? 'bg-data-cyan/10 text-data-cyan border-data-cyan/30'
            : result.engine === 'csv' || result.engine === 'mft-dump'
              ? 'bg-warning/10 text-warning border-warning/30'
              : result.engine === 'multi'
                ? 'bg-accent/10 text-accent border-accent/30'
                : 'bg-surface-bg text-text-muted border-border-default'}">
          {result.engine === "mft"
            ? t("MFT FAST SCAN")
            : result.engine === "csv"
              ? t("CSV IMPORT")
              : result.engine === "mft-dump"
                ? t("MFT DUMP")
                : result.engine === "multi"
                  ? t("MULTI-ROOT")
                  : t("TRAVERSAL FALLBACK")}
        </span>
        {#if isSnapshotView}
          <span class="text-text-muted">
            {t("Read-only snapshot. Quarantine and cleanup are disabled until you rescan the drive.")}
          </span>
        {:else if result.engine === "traversal" && isDriveRoot(primaryScanTarget)}
          <span class="text-warning">
            {t("MFT unavailable for this drive/session. Run as Administrator on local NTFS for the fast path.")}
          </span>
        {/if}
        {#if result.truncated}
          <span class="text-text-muted">{t("Tree truncated; top files/types still cover the full scan.")}</span>
        {/if}
      </div>

      <div class="flex items-center gap-2 border-b border-border-default">
        <button
          onclick={() => activeView = "treemap"}
          class="px-3 py-2 text-xs font-semibold border-b-2 transition-colors {activeView === 'treemap' ? 'border-accent text-accent' : 'border-transparent text-text-muted hover:text-text-primary'}"
        >
          {t("Treemap")}
        </button>
        <button
          onclick={() => activeView = "files"}
          class="px-3 py-2 text-xs font-semibold border-b-2 transition-colors {activeView === 'files' ? 'border-accent text-accent' : 'border-transparent text-text-muted hover:text-text-primary'}"
        >
          {t("Largest files")}
        </button>
        <button
          onclick={() => activeView = "types"}
          class="px-3 py-2 text-xs font-semibold border-b-2 transition-colors {activeView === 'types' ? 'border-accent text-accent' : 'border-transparent text-text-muted hover:text-text-primary'}"
        >
          {t("File types")}
        </button>
        <button
          onclick={() => activeView = "duplicates"}
          class="px-3 py-2 text-xs font-semibold border-b-2 transition-colors {activeView === 'duplicates' ? 'border-accent text-accent' : 'border-transparent text-text-muted hover:text-text-primary'}"
        >
          {t("Duplicates")}
          {#if duplicateMode !== "off"}
            <span class="ml-1 font-mono text-[10px] text-text-muted">({result.duplicate_groups.length})</span>
          {/if}
        </button>
        <div class="flex-1"></div>
        {#if activeView === "treemap"}
          <button
            onclick={exportTreemapPng}
            class="px-3 py-2 text-xs font-semibold text-text-secondary hover:text-text-primary"
          >
            {t("Save PNG")}
          </button>
        {/if}
        <button
          onclick={exportDiskHtml}
          class="px-3 py-2 text-xs font-semibold text-text-secondary hover:text-text-primary"
        >
          {t("Export HTML")}
        </button>
        <button
          onclick={openSelected}
          class="px-3 py-2 text-xs font-semibold text-text-secondary hover:text-text-primary flex items-center gap-1.5"
        >
          <FolderOpen class="w-3.5 h-3.5" />
          {t("Open in Explorer")}
        </button>
      </div>

      {#if activeView === "treemap"}
        <div class="flex flex-wrap items-center gap-1.5 text-xs">
          {#each focusStack as crumb, index}
            <button
              onclick={() => goToFocus(index)}
              class="px-2 py-1 rounded bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary font-mono max-w-[220px] truncate"
              title={crumb.path}
            >
              {crumb.name || crumb.path}
            </button>
            {#if index < focusStack.length - 1}<span class="text-text-muted">/</span>{/if}
          {/each}
          <span class="text-text-muted font-mono ml-2">{formatBytes(nodeWeight(focusNode || result.root))}</span>
        </div>

        <div class="flex flex-wrap items-center gap-3 text-[11px] text-text-muted">
          <label class="flex items-center gap-1.5 cursor-pointer select-none">
            <input
              type="checkbox"
              checked={treemapShowNames}
              onchange={(event) => {
                treemapShowNames = event.currentTarget.checked;
                saveDiskPreferences();
              }}
              class="rounded border-border-default text-accent focus:ring-accent bg-surface-bg cursor-pointer w-3.5 h-3.5"
            />
            {t("Labels")}
          </label>
          <div class="flex items-center rounded-md border border-border-default overflow-hidden" role="group" aria-label={t("Treemap size source")}>
            <button
              type="button"
              onclick={() => {
                treemapUseLogical = false;
                saveDiskPreferences();
              }}
              aria-pressed={!treemapUseLogical}
              class="px-2 py-1 font-semibold transition-colors {!treemapUseLogical ? 'bg-accent-subtle text-accent' : 'bg-surface-bg text-text-muted hover:text-text-primary'}"
            >
              {t("Allocated")}
            </button>
            <button
              type="button"
              onclick={() => {
                treemapUseLogical = true;
                saveDiskPreferences();
              }}
              aria-pressed={treemapUseLogical}
              class="px-2 py-1 font-semibold border-l border-border-default transition-colors {treemapUseLogical ? 'bg-accent-subtle text-accent' : 'bg-surface-bg text-text-muted hover:text-text-primary'}"
            >
              {t("Logical")}
            </button>
          </div>
          <label class="flex items-center gap-1.5 cursor-pointer select-none" title={t("Only shown when the treemap is focused on a drive root.")}>
            <input
              type="checkbox"
              checked={treemapShowFree}
              onchange={(event) => {
                treemapShowFree = event.currentTarget.checked;
                saveDiskPreferences();
              }}
              class="rounded border-border-default text-accent focus:ring-accent bg-surface-bg cursor-pointer w-3.5 h-3.5"
            />
            {t("Free space")}
          </label>
          <label class="flex items-center gap-1.5">
            {t("Group files")}
            <select
              value={treemapGroupSmall}
              onchange={(event) => {
                treemapGroupSmall = Number(event.currentTarget.value);
                saveDiskPreferences();
              }}
              class="px-1.5 py-1 rounded-md bg-surface-bg border border-border-default text-[11px] font-mono text-text-secondary outline-none cursor-pointer"
            >
              <option value={0}>{t("Off")}</option>
              <option value={1024}>{t("under 1 KB")}</option>
              <option value={65536}>{t("under 64 KB")}</option>
              <option value={1048576}>{t("under 1 MB")}</option>
            </select>
          </label>
        </div>

        <div
          class="disk-treemap relative w-full border border-border-default rounded-lg overflow-hidden bg-app-bg"
          role="img"
          aria-label={t("Treemap of {path}. Use the Largest files tab for a keyboard-accessible list.", {
            path: nodeDisplayPath(focusNode ?? result.root),
          })}
        >
          <canvas
            bind:this={canvasEl}
            onclick={handleCanvasClick}
            onmousemove={handleCanvasMove}
            onmouseleave={handleCanvasLeave}
            aria-hidden="true"
            class="block w-full h-full cursor-crosshair"
          ></canvas>

          {#if tooltip}
            <div
              class="absolute z-10 pointer-events-none max-w-[320px] rounded-md border border-border-strong bg-elevated-bg/95 px-2.5 py-2 shadow-xl"
              style="left: {tooltip.x}px; top: {tooltip.y}px"
            >
              <span class="block text-[11px] font-semibold text-text-primary break-all">{tooltip.node.name}</span>
              <span class="block text-[10px] font-mono text-text-muted mt-0.5 break-all">{nodeDisplayPath(tooltip.node)}</span>
              <span class="block text-[10px] font-mono text-data-cyan mt-1">
                {t("{size} · {percent}% · {files} files", {
                  size: formatBytes(nodeWeight(tooltip.node)),
                  percent: percentOfFocusedNode(tooltip.node),
                  files: tooltip.node.file_count.toLocaleString(),
                })}
                {#if tooltip.node.is_dir}{t("· click to zoom")}{:else}{t("· click to select")}{/if}
              </span>
              {#if !tooltip.node.is_dir && tooltip.node.modified_epoch_ms}
                <span class="block text-[10px] font-mono text-text-muted mt-1">
                  {t("Modified {date}", { date: formatModified(tooltip.node.modified_epoch_ms) })}
                </span>
              {/if}
            </div>
          {/if}
        </div>

        <div class="flex flex-wrap items-center justify-between gap-3 text-xs">
          <div class="flex items-center gap-2 text-text-muted font-mono">
            <Folder class="w-3.5 h-3.5 text-accent" />
            <span class="break-all">{nodeDisplayPath(selectedNode ?? focusNode)}</span>
          </div>
          <div class="flex items-center gap-2">
            {#if selectedNode}
              <span class="font-mono text-text-secondary">
                {t("{size} · {files} files", {
                  size: formatBytes(nodeWeight(selectedNode)),
                  files: selectedNode.file_count.toLocaleString(),
                })}
              </span>
              <button
                onclick={quarantineSelected}
                disabled={isQuarantining || isSnapshotView}
                title={isSnapshotView ? t("Snapshots are read-only") : t("Move this item to quarantine")}
                class="px-2.5 py-1.5 rounded-md bg-danger/10 hover:bg-danger-strong text-danger hover:text-white border border-danger/20 text-[11px] font-semibold transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
              >
                {isQuarantining ? t("Quarantining…") : t("Quarantine")}
              </button>
            {/if}
          </div>
        </div>

        <div class="flex flex-wrap items-center gap-3 text-[11px] text-text-muted">
          <span class="font-mono">{t("Click to zoom or select · hover for details")}</span>
          <span class="text-border-strong">/</span>
          <span class="flex items-center gap-1.5"><span class="w-2 h-2 rounded-sm" style="background: {activeDirColors[0]}"></span>{t("folder")}</span>
          {#each result.file_types.slice(0, 6) as type}
            <span class="flex items-center gap-1.5">
              <span class="w-2 h-2 rounded-sm" style="background: {colorForExtension(type.extension)}"></span>
              {type.extension || t("(none)")}
            </span>
          {/each}
        </div>
      {:else if activeView === "files"}
        <div class="flex flex-col sm:flex-row items-stretch sm:items-center gap-2">
          <div class="relative flex-1">
            <Search class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-text-muted" aria-hidden="true" />
            <input
              bind:value={fileQuery}
              oninput={scheduleFileSearch}
              placeholder={fileFilterMode === "path" ? t("Search entire file paths…") : t("Search file names…")}
              aria-label={t("Search all scanned files")}
              class="w-full pl-9 pr-3 py-2 rounded-lg bg-surface-bg border border-border-default focus:border-accent/50 focus:ring-1 focus:ring-accent/30 outline-none text-xs text-text-primary placeholder:text-text-muted"
            />
          </div>
          {#if fileExtensionFilter}
            <button
              onclick={clearExtensionFilter}
              class="px-2.5 py-2 rounded-lg bg-accent-subtle border border-accent/30 text-[11px] font-mono text-accent flex items-center gap-1.5 hover:bg-accent/20 transition-colors"
              title={t("Clear extension filter")}
            >
              .{fileExtensionFilter}
              <X class="w-3 h-3" aria-hidden="true" />
            </button>
          {/if}
          <div class="flex items-center rounded-lg border border-border-default overflow-hidden" role="group" aria-label={t("File search scope")}>
            <button
              onclick={() => setFileScope("all")}
              aria-pressed={fileScope === "all"}
              class="px-2.5 py-2 text-[11px] font-semibold transition-colors {fileScope === 'all' ? 'bg-accent-subtle text-accent' : 'bg-surface-bg text-text-muted hover:text-text-primary'}"
            >
              {t("Whole scan")}
            </button>
            <button
              onclick={() => setFileScope("folder")}
              aria-pressed={fileScope === "folder"}
              disabled={!focusNode || focusNode.path.startsWith("::")}
              title={focusNode
                ? focusNode.path.startsWith("::")
                  ? t("Open one of the scanned roots in the treemap first")
                  : t("Limit to {path}", { path: focusNode.path })
                : t("Open a folder in the treemap first")}
              class="px-2.5 py-2 text-[11px] font-semibold border-l border-border-default transition-colors disabled:opacity-40 {fileScope === 'folder' ? 'bg-accent-subtle text-accent' : 'bg-surface-bg text-text-muted hover:text-text-primary'}"
            >
              {t("This folder")}
            </button>
          </div>
          <div class="flex items-center rounded-lg border border-border-default overflow-hidden" role="group" aria-label={t("File search matching")}>
            <button
              onclick={() => setFileFilterMode("name")}
              aria-pressed={fileFilterMode === "name"}
              title={t("Match file names only; fastest")}
              class="px-2.5 py-2 text-[11px] font-semibold transition-colors {fileFilterMode === 'name' ? 'bg-accent-subtle text-accent' : 'bg-surface-bg text-text-muted hover:text-text-primary'}"
            >
              {t("Name")}
            </button>
            <button
              onclick={() => setFileFilterMode("path")}
              aria-pressed={fileFilterMode === "path"}
              title={t("Match the entire path; slower on large scans")}
              class="px-2.5 py-2 text-[11px] font-semibold border-l border-border-default transition-colors {fileFilterMode === 'path' ? 'bg-accent-subtle text-accent' : 'bg-surface-bg text-text-muted hover:text-text-primary'}"
            >
              {t("Path")}
            </button>
          </div>
          <label class="flex items-center gap-1.5 text-[11px] font-mono text-text-muted">
            {t("Max")}
            <select
              bind:value={fileLimit}
              onchange={() => { scheduleFileSearch(); saveDiskPreferences(); }}
              aria-label={t("Maximum files to display")}
              class="px-2 py-2 rounded-lg bg-surface-bg border border-border-default text-[11px] font-mono text-text-secondary outline-none cursor-pointer"
            >
              <option value={100}>100</option>
              <option value={500}>500</option>
              <option value={1000}>1000</option>
              <option value={2000}>2000</option>
            </select>
          </label>
          <button
            onclick={exportTopFilesCsv}
            class="px-3 py-2 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-xs font-semibold text-text-secondary hover:text-text-primary transition-colors"
            title={t("Export the files currently shown")}
          >
            {t("Export view")}
          </button>
          <button
            onclick={exportAllFilesCsv}
            class="px-3 py-2 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-xs font-semibold text-text-secondary hover:text-text-primary transition-colors"
            title={t("Export every indexed file matching the current filters")}
          >
            {t("Export all")}
          </button>
          <span class="text-[11px] font-mono text-text-muted self-center" aria-live="polite">
            {#if isSearching}
              {t("Searching…")}
            {:else if isSearchActive && searchResult}
              {t("{matches} matches · {indexed} indexed", {
                matches: searchResult.total_matches.toLocaleString(),
                indexed: searchResult.indexed_files.toLocaleString(),
              })}{searchResult.index_complete ? "" : ` ${t("(capped)")}`}
              {#if searchResult.total_matches > searchResult.files.length}
                · {t("top {count} by {sort}", {
                  count: searchResult.files.length,
                  sort: fileSortLabel,
                })}
              {/if}
            {:else}
              {t("Top {count} files · search covers all scanned files", { count: displayFiles.length })}
            {/if}
          </span>
        </div>

        <div class="flex flex-col lg:flex-row lg:items-center gap-2">
          <span class="text-[11px] font-mono uppercase tracking-wider text-text-muted shrink-0">{t("Path filters")}</span>
          <label class="relative flex-1 min-w-0">
            <span class="absolute left-3 top-1/2 -translate-y-1/2 font-mono text-xs font-bold text-success" aria-hidden="true">+</span>
            <input
              value={fileInclude}
              oninput={(event) => { fileInclude = event.currentTarget.value; updatePathFilters(); }}
              placeholder={t("Include paths containing…")}
              aria-label={t("Include only file paths containing")}
              class="w-full pl-8 pr-8 py-2 rounded-lg bg-surface-bg border border-border-default focus:border-success/50 focus:ring-1 focus:ring-success/30 outline-none text-xs text-text-primary placeholder:text-text-muted"
            />
            {#if fileInclude}
              <button
                type="button"
                onclick={() => { fileInclude = ""; updatePathFilters(); }}
                class="absolute right-2 top-1/2 -translate-y-1/2 p-1 rounded text-text-muted hover:text-text-primary transition-colors"
                title={t("Clear include filter")}
                aria-label={t("Clear include filter")}
              >
                <X class="w-3 h-3" aria-hidden="true" />
              </button>
            {/if}
          </label>
          <label class="relative flex-1 min-w-0">
            <span class="absolute left-3 top-1/2 -translate-y-1/2 font-mono text-xs font-bold text-danger" aria-hidden="true">−</span>
            <input
              value={fileExclude}
              oninput={(event) => { fileExclude = event.currentTarget.value; updatePathFilters(); }}
              placeholder={t("Exclude paths containing…")}
              aria-label={t("Exclude file paths containing")}
              class="w-full pl-8 pr-8 py-2 rounded-lg bg-surface-bg border border-border-default focus:border-danger/50 focus:ring-1 focus:ring-danger/30 outline-none text-xs text-text-primary placeholder:text-text-muted"
            />
            {#if fileExclude}
              <button
                type="button"
                onclick={() => { fileExclude = ""; updatePathFilters(); }}
                class="absolute right-2 top-1/2 -translate-y-1/2 p-1 rounded text-text-muted hover:text-text-primary transition-colors"
                title={t("Clear exclude filter")}
                aria-label={t("Clear exclude filter")}
              >
                <X class="w-3 h-3" aria-hidden="true" />
              </button>
            {/if}
          </label>
          {#if fileInclude || fileExclude}
            <button
              type="button"
              onclick={clearPathFilters}
              class="px-3 py-2 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-[11px] font-semibold text-text-secondary hover:text-text-primary transition-colors shrink-0"
            >
              {t("Clear filters")}
            </button>
          {/if}
          <span class="text-[10px] font-mono text-text-muted shrink-0">{t("case-insensitive substring · full path")}</span>
        </div>

        <div class="border border-border-default rounded-lg overflow-hidden">
          <div class="overflow-x-auto">
            <table class="w-full text-left text-xs min-w-[960px]">
              <thead class="bg-sidebar-bg text-text-muted uppercase tracking-wider font-mono sticky top-0 z-10">
                <tr>
                  <th class="px-4 py-3" aria-sort={fileSort === "name" ? "ascending" : "none"}>
                    <button
                      type="button"
                      onclick={() => setFileSort("name")}
                      title={t("Sort by file name")}
                      class="inline-flex items-center gap-1 uppercase hover:text-text-primary transition-colors"
                    >
                      {t("File")}
                      {#if fileSort === "name"}<ArrowUp class="w-3 h-3 text-accent" aria-hidden="true" />{/if}
                    </button>
                  </th>
                  <th class="px-4 py-3">{t("Extension")}</th>
                  <th class="px-4 py-3" aria-sort={fileSort === "modified" ? "descending" : "none"}>
                    <button
                      type="button"
                      onclick={() => setFileSort("modified")}
                      title={t("Sort by last modified, newest first")}
                      class="inline-flex items-center gap-1 uppercase hover:text-text-primary transition-colors"
                    >
                      {t("Modified")}
                      {#if fileSort === "modified"}<ArrowDown class="w-3 h-3 text-accent" aria-hidden="true" />{/if}
                    </button>
                  </th>
                  <th class="px-4 py-3">{t("Attributes")}</th>
                  <th class="px-4 py-3 text-right">{t("Share")}</th>
                  <th class="px-4 py-3 text-right" aria-sort={fileSort === "size" ? "descending" : "none"}>
                    <button
                      type="button"
                      onclick={() => setFileSort("size")}
                      title={t("Sort by allocated size, largest first")}
                      class="inline-flex items-center gap-1 uppercase hover:text-text-primary transition-colors"
                    >
                      {t("Allocated")}
                      {#if fileSort === "size"}<ArrowDown class="w-3 h-3 text-accent" aria-hidden="true" />{/if}
                    </button>
                  </th>
                  <th class="px-4 py-3 text-right">{t("Logical")}</th>
                  {#if result.duplicate_mode !== "off"}
                    <th class="px-4 py-3 text-right">{t("Dup Count")}</th>
                    <th class="px-4 py-3 text-right">{t("Dup Waste")}</th>
                  {/if}
                </tr>
              </thead>
              <tbody class="divide-y divide-border-default">
                {#each displayFiles as file}
                  <tr class="hover:bg-elevated-bg/50 cursor-pointer" onclick={() => openFileFolder(file)}>
                    <td class="px-4 py-2.5">
                      <button
                        type="button"
                        onclick={(event) => { event.stopPropagation(); openFileFolder(file); }}
                        class="block w-full text-left rounded focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent"
                        title={t("Open containing folder")}
                        aria-label={t("Open folder containing {name}", { name: file.name })}
                      >
                        <span class="text-text-primary font-mono break-all">{file.name}</span>
                        <span class="block text-[11px] text-text-muted font-mono truncate max-w-[420px]" title={file.path}>{file.path}</span>
                      </button>
                    </td>
                    <td class="px-4 py-2.5 font-mono text-text-secondary">{file.extension}</td>
                    <td class="px-4 py-2.5 font-mono text-text-muted whitespace-nowrap">{formatModified(file.modified_epoch_ms)}</td>
                    <td class="px-4 py-2.5 font-mono text-text-muted whitespace-nowrap" title={t("Read-only, Hidden, System, Directory, Archive, Temporary, Sparse, Compressed, Offline, Encrypted, Integrity, Not-indexed")}>{attributesLabel(file.attributes)}</td>
                    <td class="px-4 py-2.5 text-right font-mono text-data-cyan">{fileSharePercent(file)}%</td>
                    <td class="px-4 py-2.5 text-right font-mono text-text-primary">{formatBytes(file.allocated_bytes)}</td>
                    <td class="px-4 py-2.5 text-right font-mono text-text-muted">{formatBytes(file.size_bytes)}</td>
                    {#if result.duplicate_mode !== "off"}
                      <td class="px-4 py-2.5 text-right font-mono text-text-muted">
                        {duplicateStats.get(file.path)?.count ?? "—"}
                      </td>
                      <td class="px-4 py-2.5 text-right font-mono text-text-muted">
                        {duplicateStats.get(file.path) ? formatBytes(duplicateStats.get(file.path)!.waste) : "—"}
                      </td>
                    {/if}
                  </tr>
                {:else}
                  <tr>
                    <td colspan={result.duplicate_mode !== "off" ? 9 : 7} class="px-4 py-8 text-center text-text-muted">
                      {isSearching
                        ? t("Searching indexed files…")
                        : isSearchActive
                          ? t("No scanned files match this search.")
                          : t("No top files match this filter.")}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>
      {:else if activeView === "types"}
        <div class="border border-border-default rounded-lg bg-surface-bg/40 p-2 space-y-1">
          {#each result.file_types as type}
            <button
              type="button"
              onclick={() => showFilesByExtension(type.extension)}
              title={type.extension
                ? t("Show all .{ext} files by size", { ext: type.extension })
                : t("Show all files with no extension by size")}
              class="w-full text-left p-2 rounded-md hover:bg-elevated-bg/70 focus-visible:bg-elevated-bg/70 transition-colors"
            >
              <div class="flex items-center justify-between text-xs font-mono gap-3">
                <span class="flex items-center gap-2 text-text-primary">
                  <span class="w-2.5 h-2.5 rounded-sm" style="background: {colorForExtension(type.extension)}"></span>
                  {type.extension || t("(none)")}
                </span>
                <span class="text-text-muted">
                  {t("{size} · {count} files", {
                    size: formatBytes(type.size_bytes),
                    count: type.file_count.toLocaleString(),
                  })} ·
                  {result.total_allocated > 0 ? ((type.size_bytes / result.total_allocated) * 100).toFixed(1) : "0.0"}%
                </span>
              </div>
              <div class="h-2 bg-app-bg rounded-full overflow-hidden mt-1">
                <div
                  class="h-full rounded-full"
                  style="width: {result.total_allocated > 0 ? Math.max(1, (type.size_bytes / result.total_allocated) * 100) : 0}%; background: {colorForExtension(type.extension)}"
                ></div>
              </div>
            </button>
          {/each}
        </div>
      {:else}
        {#if !lastScanHadDuplicates}
          <div class="p-6 border border-dashed border-border-default rounded-lg bg-surface-bg/20 text-center text-xs text-text-muted">
            {t("Enable")} <b class="text-text-secondary">{t("Duplicates")}</b> {t("and scan again to compare by name + size or name + size + date.")}
          </div>
        {:else if result.duplicate_groups.length === 0}
          <div class="p-6 border border-dashed border-border-default rounded-lg bg-surface-bg/20 text-center text-xs text-text-muted">
            {t("No duplicate groups found ({criteria}).", {
              criteria: result.duplicate_mode === "name_size_date"
                ? t("same name, size and modified date")
                : t("same name and size"),
            })}
          </div>
        {:else}
          <div class="flex flex-wrap items-center justify-between gap-3 instrument-panel px-3.5 py-2.5">
            <span class="text-[11px] font-mono text-text-muted" aria-live="polite">
              {#if isSnapshotView}
                {t("Read-only snapshot · rescan the drive to clean duplicates")}
              {:else}
                {t("{count} selected · {size} reclaimable", {
                  count: selectedDuplicateCount,
                  size: formatBytes(selectedDuplicateBytes),
                })}
              {/if}
            </span>
            {#if !isSnapshotView}
              <div class="flex items-center gap-2">
                <button
                  onclick={clearDuplicateSelection}
                  disabled={selectedDuplicateCount === 0 || isQuarantining}
                  class="px-2.5 py-1.5 rounded-md bg-surface-bg border border-border-default hover:bg-elevated-bg text-[11px] font-semibold text-text-secondary disabled:opacity-40 transition-colors"
                >
                  {t("Clear")}
                </button>
                <button
                  onclick={quarantineSelectedDuplicates}
                  disabled={selectedDuplicateCount === 0 || isQuarantining}
                  class="px-2.5 py-1.5 rounded-md bg-danger/10 hover:bg-danger-strong text-danger hover:text-white border border-danger/20 text-[11px] font-semibold disabled:opacity-40 transition-colors"
                >
                  {isQuarantining ? t("Quarantining…") : t("Quarantine {count}", { count: selectedDuplicateCount })}
                </button>
              </div>
            {/if}
          </div>
          <div class="p-3 rounded-lg bg-warning/10 border border-warning/20 text-[11px] text-warning">
            {t("Groups match by file name and size (plus modified date if enabled). File contents are not compared; verify the files before purging.")}
          </div>
          <div class="space-y-3">
            {#each result.duplicate_groups as group}
              <div class="instrument-panel p-3.5 space-y-2">
                <div class="flex flex-wrap items-center justify-between gap-2">
                  <div class="flex items-center gap-2 min-w-0">
                    <File class="w-4 h-4 text-text-muted flex-shrink-0" />
                    <span class="text-sm font-semibold text-text-primary truncate" title={group.name}>{group.name}</span>
                    <span class="text-[10px] font-mono text-text-muted">{t("{count} copies", { count: group.file_count })}</span>
                    {#if group.files.length > 1}
                      <button
                        onclick={() => selectAllButFirst(group)}
                        disabled={isQuarantining || isSnapshotView}
                        class="text-[10px] font-mono text-text-muted hover:text-accent disabled:opacity-40 transition-colors"
                        title={t("Select every listed copy except the first")}
                      >
                        {t("Select rest")}
                      </button>
                    {/if}
                  </div>
                  <span class="readout text-xs text-data-cyan">
                    {t("{size} each · {waste} waste", {
                      size: formatBytes(group.size_bytes),
                      waste: formatBytes(group.size_bytes * (group.file_count - 1)),
                    })}
                  </span>
                </div>
                <div class="space-y-1">
                  {#each group.files as file}
                    <div class="flex items-center gap-2 px-2 py-1 rounded bg-app-bg/60 hover:bg-elevated-bg transition-colors">
                      <input
                        type="checkbox"
                        checked={!!duplicateSelection[file.path]}
                        disabled={isSnapshotView || isQuarantining || (!duplicateSelection[file.path] && duplicatesSelectedInGroup(group) >= group.file_count - 1)}
                        onchange={() => toggleDuplicateFile(group, file)}
                        aria-label={t("Select duplicate copy at {path}", { path: file.path })}
                        title={duplicateSelection[file.path] ? t("Do not quarantine this copy") : t("Quarantine this copy")}
                        class="rounded border-border-default text-accent focus:ring-accent bg-surface-bg cursor-pointer w-3.5 h-3.5 flex-shrink-0 disabled:opacity-30 disabled:cursor-not-allowed"
                      />
                      <button
                        onclick={() => openFileFolder(file)}
                        class="flex-1 text-left text-[11px] font-mono text-text-muted hover:text-text-primary transition-colors break-all"
                        title={file.path}
                      >
                        {file.path}
                      </button>
                    </div>
                  {/each}
                  {#if group.file_count > group.files.length}
                    <span class="block text-[10px] text-text-muted px-2">{t("+{count} more copies", { count: group.file_count - group.files.length })}</span>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        {/if}
      {/if}
    {:else}
      <div class="flex flex-col items-center justify-center h-80 border border-dashed border-border-default rounded-xl bg-surface-bg/20 text-center gap-3">
        <HardDrive class="w-10 h-10 text-text-muted/60" />
        <div>
          <h3 class="text-sm font-bold text-text-primary">{t("Scan a drive or folder")}</h3>
          <p class="text-xs text-text-muted mt-1 max-w-md leading-relaxed">
            {t("PurgeKit reads the selected tree once, shows allocated size (sparse/compressed aware), and lets you drill into the largest folders and files. Symbols, junctions and reparse points are skipped.")}
          </p>
        </div>
        <div class="flex items-center gap-2 text-[10px] text-text-muted font-mono">
          <File class="w-3.5 h-3.5" /> {t("top files")}
          <Folder class="w-3.5 h-3.5" /> {t("treemap")}
          <HardDrive class="w-3.5 h-3.5" /> {t("allocated size")}
        </div>
      </div>
    {/if}
  </div>
</div>
