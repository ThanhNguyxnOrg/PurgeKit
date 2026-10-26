<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { FolderOpen, Play, Square, HardDrive, File, Folder, Search, X } from "@lucide/svelte";
  import { toast } from "../toast.svelte";

  interface DiskTreeNode {
    name: string;
    path: string;
    is_dir: boolean;
    size_bytes: number;
    allocated_bytes: number;
    file_count: number;
    children: DiskTreeNode[];
  }

  interface DiskFile {
    name: string;
    path: string;
    extension: string;
    size_bytes: number;
    allocated_bytes: number;
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
    engine: "mft" | "traversal";
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

  interface DriveInfo {
    letter: string;
    label: string;
    kind: "fixed" | "removable";
    total_bytes: number;
    free_bytes: number;
  }

  interface LayoutRect {
    node: DiskTreeNode;
    x: number;
    y: number;
    w: number;
    h: number;
  }

  let pathInput = $state("C:\\");
  let drives = $state<DriveInfo[]>([]);
  let result = $state<DiskScanResult | null>(null);
  let focusNode = $state<DiskTreeNode | null>(null);
  let focusStack = $state<DiskTreeNode[]>([]);
  let selectedNode = $state<DiskTreeNode | null>(null);
  let activeView = $state<"treemap" | "files" | "types" | "duplicates">("treemap");
  let findDuplicates = $state(false);
  let lastScanHadDuplicates = $state(false);
  let isScanning = $state(false);
  let isCancelling = $state(false);
  let progress = $state<DiskProgress | null>(null);
  let errorMessage = $state("");
  let canvasEl = $state<HTMLCanvasElement | null>(null);
  let layoutRects: LayoutRect[] = [];
  let hoverPath = $state<string | null>(null);
  let tooltip = $state<{ x: number; y: number; node: DiskTreeNode } | null>(null);
  let fileQuery = $state("");
  let fileExtensionFilter = $state<string | null>(null);
  let fileScope = $state<"all" | "folder">("all");
  let searchResult = $state<DiskFileSearchResult | null>(null);
  let isSearching = $state(false);
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let searchSequence = 0;
  let isQuarantining = $state(false);

  let filteredTopFiles = $derived(
    result
      ? result.top_files.filter((file) => {
          const query = fileQuery.trim().toLowerCase();
          return !query ||
            file.name.toLowerCase().includes(query) ||
            file.path.toLowerCase().includes(query) ||
            file.extension.toLowerCase().includes(query);
        })
      : []
  );

  let displayFiles = $derived(
    searchResult ? searchResult.files : filteredTopFiles
  );

  let isSearchActive = $derived(
    fileQuery.trim().length > 0 ||
      fileExtensionFilter !== null ||
      (fileScope === "folder" && focusNode !== null)
  );

  const DIR_COLORS = ["#33545A", "#3E5364", "#4B465E", "#5A4B43", "#3C5747", "#45566B"];
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
    return EXT_COLORS[extensionGroup(extension)] || EXT_COLORS.other;
  }

  function formatBytes(bytes: number): string {
    if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    const index = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
    return `${(bytes / Math.pow(1024, index)).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
  }

  function nodeWeight(node: DiskTreeNode): number {
    return Math.max(node.allocated_bytes, node.size_bytes);
  }

  function isDriveRoot(value: string): boolean {
    return /^[a-zA-Z]:[\\/]*$/.test(value.trim());
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
        prefix: fileScope === "folder" ? focusNode?.path ?? null : null,
        limit: 500,
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
    scheduleFileSearch();
  }

  async function scanDisk() {
    const target = pathInput.trim();
    if (!target) {
      toast.show("Enter a drive or folder path first.", "warning");
      return;
    }

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
    clearFileSearch();

    try {
      const scanResult = await invoke<DiskScanResult>("scan_disk_space", {
        path: target,
        maxDepth: 4,
        maxChildren: 40,
        findDuplicates,
      });
      lastScanHadDuplicates = findDuplicates;
      result = scanResult;
      focusStack = [scanResult.root];
      focusNode = scanResult.root;
      activeView = "treemap";
      toast.show(
        `Scanned ${scanResult.total_files.toLocaleString()} files in ${(scanResult.elapsed_ms / 1000).toFixed(1)}s.`,
        "success"
      );
    } catch (e: any) {
      const message = e.toString();
      if (message.toLowerCase().includes("cancelled")) {
        errorMessage = "Scan cancelled.";
      } else {
        errorMessage = message;
        toast.show(`Disk scan failed: ${message}`, "error");
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
      output.push({ node: items[0], x, y, w, h });
      return output;
    }

    const total = items.reduce((sum, item) => sum + nodeWeight(item), 0);
    if (total <= 0) return output;

    let accumulated = 0;
    let splitIndex = 1;
    for (let index = 0; index < items.length - 1; index++) {
      accumulated += nodeWeight(items[index]);
      if (accumulated >= total / 2) {
        splitIndex = index + 1;
        break;
      }
    }

    const first = items.slice(0, splitIndex);
    const second = items.slice(splitIndex);
    const firstTotal = first.reduce((sum, item) => sum + nodeWeight(item), 0);
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

  function colorForNode(node: DiskTreeNode, index: number): string {
    let hash = 0;
    for (const character of node.name) {
      hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
    }
    if (node.is_dir) {
      return DIR_COLORS[(hash + index) % DIR_COLORS.length];
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
    const height = 520;
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

    const children = [...node.children]
      .filter((child) => nodeWeight(child) > 0)
      .sort((left, right) => nodeWeight(right) - nodeWeight(left));

    if (children.length === 0) {
      context.fillStyle = "#8a94a3";
      context.font = "13px system-ui, Segoe UI, sans-serif";
      context.textAlign = "center";
      context.fillText("No deeper detail for this folder.", width / 2, height / 2 - 8);
      context.font = "11px ui-monospace, Consolas, monospace";
      context.fillText(node.path, width / 2, height / 2 + 14, width - 32);
      layoutRects = [];
      return;
    }

    layoutRects = binaryTreemap(children, 2, 2, width - 4, height - 4);
    layoutRects.forEach((rect, index) => {
      const highlighted =
        rect.node.path === hoverPath || rect.node.path === selectedNode?.path;
      context.fillStyle = colorForNode(rect.node, index);
      context.fillRect(rect.x, rect.y, rect.w, rect.h);

      context.strokeStyle = highlighted ? "#F4F5F7" : "rgba(9, 11, 15, 0.85)";
      context.lineWidth = highlighted ? 2 : 1;
      context.strokeRect(rect.x + 0.5, rect.y + 0.5, Math.max(0, rect.w - 1), Math.max(0, rect.h - 1));

      if (rect.w > 58 && rect.h > 30) {
        context.save();
        context.beginPath();
        context.rect(rect.x + 3, rect.y + 3, rect.w - 6, rect.h - 6);
        context.clip();
        context.fillStyle = "#f4f6f8";
        context.font = "11px ui-monospace, Consolas, monospace";
        context.textAlign = "left";
        context.fillText(rect.node.name, rect.x + 7, rect.y + 18, rect.w - 14);
        context.fillStyle = "rgba(244, 246, 248, 0.7)";
        context.font = "10px ui-monospace, Consolas, monospace";
        context.fillText(formatBytes(nodeWeight(rect.node)), rect.x + 7, rect.y + 33, rect.w - 14);
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
    if (!hit) return;

    selectedNode = hit.node;
    if (hit.node.is_dir && hit.node.children.length > 0) {
      focusStack = [...focusStack, hit.node];
      focusNode = hit.node;
    }
  }

  function rectAt(x: number, y: number): LayoutRect | undefined {
    return layoutRects.find(
      (rect) => x >= rect.x && x <= rect.x + rect.w && y >= rect.y && y <= rect.y + rect.h
    );
  }

  function handleCanvasMove(event: MouseEvent) {
    const canvas = canvasEl;
    if (!canvas) return;
    const bounds = canvas.getBoundingClientRect();
    const x = event.clientX - bounds.left;
    const y = event.clientY - bounds.top;
    const hit = rectAt(x, y);
    hoverPath = hit?.node.path || null;
    tooltip = hit ? { x, y, node: hit.node } : null;
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
      toast.show("No files to export for this filter.", "warning");
      return;
    }

    const escapeCsv = (value: string) => `"${value.replace(/"/g, '""')}"`;
    const rows = [
      ["Name", "Path", "Extension", "LogicalBytes", "AllocatedBytes"],
      ...displayFiles.map((file) => [
        file.name,
        file.path,
        file.extension,
        String(file.size_bytes),
        String(file.allocated_bytes),
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

  async function quarantineSelected() {
    const node = selectedNode;
    if (!node || !result) return;
    if (node.path === result.root_path) {
      toast.show("Select a child folder or file first; the scan root is protected.", "warning");
      return;
    }

    const confirmed = confirm(
      `Quarantine "${node.name}"?\n\n` +
      `${node.path}\n\n` +
      `The item is moved to PurgeKit quarantine and can be restored from Settings. ` +
      `The scan result will be cleared.`
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
          `Quarantine failed: ${response.errors?.[0] || "see logs"}`,
          "error"
        );
        return;
      }

      toast.show(`Quarantined ${node.name}.`, "success");
      result = null;
      focusNode = null;
      focusStack = [];
      selectedNode = null;
      hoverPath = null;
      tooltip = null;
      layoutRects = [];
    } catch (e: any) {
      toast.show(`Quarantine failed: ${e.toString()}`, "error");
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
    const folder = node.is_dir
      ? node.path
      : node.path.slice(0, Math.max(0, node.path.lastIndexOf("\\")));
    try {
      await openPath(folder);
    } catch (e: any) {
      toast.show(`Could not open folder: ${e.toString()}`, "error");
    }
  }

  async function openFileFolder(file: DiskFile) {
    const folder = file.path.slice(0, Math.max(0, file.path.lastIndexOf("\\")));
    try {
      await openPath(folder);
    } catch (e: any) {
      toast.show(`Could not open folder: ${e.toString()}`, "error");
    }
  }

  $effect(() => {
    // Track the values that affect the canvas, then redraw after layout.
    focusNode;
    activeView;
    canvasEl;
    hoverPath;
    selectedNode;
    if (canvasEl && activeView === "treemap") {
      requestAnimationFrame(drawTreemap);
    }
  });

  $effect(() => {
    if (activeView === "files" && fileScope === "folder") {
      focusNode;
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
    invoke<DriveInfo[]>("list_disk_drives")
      .then((list) => { drives = list; })
      .catch(() => {});

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
          Disk Space
        </h2>
        <p class="text-xs text-text-muted mt-1">
          Fast folder scan with allocated size, tree map, largest files and file types.
        </p>
      </div>
      <div class="flex items-center gap-2">
        {#if isScanning}
          <button
            onclick={cancelScan}
            disabled={isCancelling}
            class="px-3 py-2 rounded-lg bg-danger/10 hover:bg-danger text-danger hover:text-white border border-danger/20 text-xs font-semibold transition-all disabled:opacity-40"
          >
            <Square class="w-3.5 h-3.5 inline mr-1" />
            {isCancelling ? "Stopping…" : "Cancel"}
          </button>
        {:else}
          <button
            onclick={scanDisk}
            class="flex items-center gap-2 px-4 py-2 rounded-lg bg-accent hover:bg-accent-hover text-white text-xs font-semibold transition-all active:scale-95"
          >
            <Play class="w-3.5 h-3.5" />
            Scan
          </button>
        {/if}
      </div>
    </div>

    <div class="flex flex-col gap-2">
      <div class="relative">
        <input
          bind:value={pathInput}
          onkeydown={(event) => event.key === "Enter" && !isScanning && scanDisk()}
          placeholder="C:\ or D:\Code"
          aria-label="Drive or folder to scan"
          class="w-full px-3 py-2 rounded-lg bg-surface-bg border border-border-default focus:border-accent/50 focus:ring-1 focus:ring-accent/30 outline-none text-sm font-mono text-text-primary placeholder:text-text-muted"
        />
      </div>
      <div class="flex flex-wrap items-center gap-2">
        {#each drives as drive}
          <button
            onclick={() => { pathInput = `${drive.letter}:\\`; scanDisk(); }}
            disabled={isScanning}
            title="{drive.label || 'Local Disk'} · {formatBytes(drive.free_bytes)} free of {formatBytes(drive.total_bytes)}"
            aria-label="Scan drive {drive.letter}:"
            class="flex items-center gap-1.5 px-2.5 py-2 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-[11px] font-semibold text-text-secondary disabled:opacity-40 transition-colors"
          >
            <HardDrive class="w-3.5 h-3.5 text-text-muted" aria-hidden="true" />
            <span class="font-mono text-text-primary">{drive.letter}:</span>
            <span class="font-mono text-text-muted">{formatBytes(drive.free_bytes)} free</span>
          </button>
        {:else}
          <button
            onclick={() => { pathInput = "C:\\"; scanDisk(); }}
            disabled={isScanning}
            class="px-3 py-2 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-xs font-semibold text-text-secondary disabled:opacity-40"
          >
            C:\
          </button>
        {/each}
        <label class="flex items-center gap-2 px-3 py-2 rounded-lg bg-surface-bg border border-border-default text-xs font-semibold text-text-secondary cursor-pointer select-none">
          <input
            type="checkbox"
            bind:checked={findDuplicates}
            disabled={isScanning}
            class="rounded border-border-default text-accent focus:ring-accent bg-surface-bg cursor-pointer w-3.5 h-3.5"
          />
          Duplicates
        </label>
      </div>
    </div>

    {#if isScanning}
      <div class="p-3 rounded-lg bg-accent/5 border border-accent/15 text-xs">
        <div class="flex items-center justify-between gap-3">
          <span class="text-text-secondary">Scanning…</span>
          <span class="font-mono text-accent">
            {progress ? `${progress.files.toLocaleString()} files · ${progress.dirs.toLocaleString()} dirs · ${formatBytes(progress.bytes)}` : "starting"}
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
          <span class="readout-label">Allocated</span>
          <p class="readout text-2xl font-semibold text-data-cyan mt-1">{formatBytes(result.total_allocated)}</p>
        </div>
        <div class="px-4 py-3.5">
          <span class="readout-label">Logical</span>
          <p class="readout text-2xl font-semibold text-text-primary mt-1">{formatBytes(result.total_size)}</p>
        </div>
        <div class="px-4 py-3.5">
          <span class="readout-label">Files / folders</span>
          <p class="readout text-2xl font-semibold text-text-primary mt-1">
            {result.total_files.toLocaleString()}
            <span class="text-base text-text-muted"> / {result.total_dirs.toLocaleString()}</span>
          </p>
        </div>
        <div class="px-4 py-3.5">
          <span class="readout-label">Scan time</span>
          <p class="readout text-2xl font-semibold text-text-primary mt-1">
            {(result.elapsed_ms / 1000).toFixed(2)}<span class="text-base text-text-muted">s</span>
          </p>
          {#if result.errors > 0}
            <p class="text-[11px] text-warning mt-1">{result.errors} entries skipped</p>
          {/if}
        </div>
        <div class="px-4 py-3.5">
          <span class="readout-label">Volume free</span>
          <p class="readout text-2xl font-semibold text-data-green mt-1">{formatBytes(result.volume_free_bytes)}</p>
          <p class="text-[11px] text-text-muted mt-1">of {formatBytes(result.volume_total_bytes)}</p>
        </div>
      </div>

      <div class="flex flex-wrap items-center gap-2 text-[11px]">
        <span class="px-2 py-1 rounded border font-mono
          {result.engine === 'mft'
            ? 'bg-data-cyan/10 text-data-cyan border-data-cyan/30'
            : 'bg-surface-bg text-text-muted border-border-default'}">
          {result.engine === "mft" ? "MFT FAST SCAN" : "TRAVERSAL FALLBACK"}
        </span>
        {#if result.engine !== "mft" && isDriveRoot(pathInput)}
          <span class="text-warning">
            MFT unavailable for this drive/session. Run as Administrator on local NTFS for the fast path.
          </span>
        {/if}
        {#if result.truncated}
          <span class="text-text-muted">Tree truncated; top files/types still cover the full scan.</span>
        {/if}
      </div>

      <div class="flex items-center gap-2 border-b border-border-default">
        <button
          onclick={() => activeView = "treemap"}
          class="px-3 py-2 text-xs font-semibold border-b-2 transition-colors {activeView === 'treemap' ? 'border-accent text-accent' : 'border-transparent text-text-muted hover:text-text-primary'}"
        >
          Treemap
        </button>
        <button
          onclick={() => activeView = "files"}
          class="px-3 py-2 text-xs font-semibold border-b-2 transition-colors {activeView === 'files' ? 'border-accent text-accent' : 'border-transparent text-text-muted hover:text-text-primary'}"
        >
          Largest files
        </button>
        <button
          onclick={() => activeView = "types"}
          class="px-3 py-2 text-xs font-semibold border-b-2 transition-colors {activeView === 'types' ? 'border-accent text-accent' : 'border-transparent text-text-muted hover:text-text-primary'}"
        >
          File types
        </button>
        <button
          onclick={() => activeView = "duplicates"}
          class="px-3 py-2 text-xs font-semibold border-b-2 transition-colors {activeView === 'duplicates' ? 'border-accent text-accent' : 'border-transparent text-text-muted hover:text-text-primary'}"
        >
          Duplicates
          {#if findDuplicates}
            <span class="ml-1 font-mono text-[10px] text-text-muted">({result.duplicate_groups.length})</span>
          {/if}
        </button>
        <div class="flex-1"></div>
        {#if activeView === "treemap"}
          <button
            onclick={exportTreemapPng}
            class="px-3 py-2 text-xs font-semibold text-text-secondary hover:text-text-primary"
          >
            Save PNG
          </button>
        {/if}
        <button
          onclick={openSelected}
          class="px-3 py-2 text-xs font-semibold text-text-secondary hover:text-text-primary flex items-center gap-1.5"
        >
          <FolderOpen class="w-3.5 h-3.5" />
          Open in Explorer
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

        <div
          class="relative w-full h-[520px] border border-border-default rounded-lg overflow-hidden bg-app-bg"
          role="img"
          aria-label="Treemap of {focusNode ? focusNode.path : result.root_path}. Use the Largest files tab for a keyboard-accessible list."
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
              style="left: {tooltip.x + 12}px; top: {tooltip.y + 12}px"
            >
              <span class="block text-[11px] font-semibold text-text-primary break-all">{tooltip.node.name}</span>
              <span class="block text-[10px] font-mono text-text-muted mt-0.5 break-all">{tooltip.node.path}</span>
              <span class="block text-[10px] font-mono text-data-cyan mt-1">
                {formatBytes(nodeWeight(tooltip.node))} · {tooltip.node.file_count.toLocaleString()} files
                {#if tooltip.node.is_dir}· click to zoom{:else}· click to select{/if}
              </span>
            </div>
          {/if}
        </div>

        <div class="flex flex-wrap items-center justify-between gap-3 text-xs">
          <div class="flex items-center gap-2 text-text-muted font-mono">
            <Folder class="w-3.5 h-3.5 text-accent" />
            <span class="break-all">{selectedNode ? selectedNode.path : (focusNode ? focusNode.path : "")}</span>
          </div>
          <div class="flex items-center gap-2">
            {#if selectedNode}
              <span class="font-mono text-text-secondary">
                {formatBytes(nodeWeight(selectedNode))} · {selectedNode.file_count.toLocaleString()} files
              </span>
              <button
                onclick={quarantineSelected}
                disabled={isQuarantining}
                class="px-2.5 py-1.5 rounded-md bg-danger/10 hover:bg-danger text-danger hover:text-white border border-danger/20 text-[11px] font-semibold transition-colors disabled:opacity-40"
              >
                {isQuarantining ? "Quarantining…" : "Quarantine"}
              </button>
            {/if}
          </div>
        </div>

        <div class="flex flex-wrap items-center gap-3 text-[11px] text-text-muted">
          <span class="font-mono">Click to zoom or select · hover for details</span>
          <span class="text-border-strong">/</span>
          <span class="flex items-center gap-1.5"><span class="w-2 h-2 rounded-sm" style="background: {DIR_COLORS[0]}"></span>folder</span>
          {#each result.file_types.slice(0, 6) as type}
            <span class="flex items-center gap-1.5">
              <span class="w-2 h-2 rounded-sm" style="background: {colorForExtension(type.extension)}"></span>
              {type.extension || "(none)"}
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
              placeholder="Search every scanned file by name or path…"
              aria-label="Search all scanned files"
              class="w-full pl-9 pr-3 py-2 rounded-lg bg-surface-bg border border-border-default focus:border-accent/50 focus:ring-1 focus:ring-accent/30 outline-none text-xs text-text-primary placeholder:text-text-muted"
            />
          </div>
          {#if fileExtensionFilter}
            <button
              onclick={clearExtensionFilter}
              class="px-2.5 py-2 rounded-lg bg-accent-subtle border border-accent/30 text-[11px] font-mono text-accent flex items-center gap-1.5 hover:bg-accent/20 transition-colors"
              title="Clear extension filter"
            >
              .{fileExtensionFilter}
              <X class="w-3 h-3" aria-hidden="true" />
            </button>
          {/if}
          <div class="flex items-center rounded-lg border border-border-default overflow-hidden" role="group" aria-label="File search scope">
            <button
              onclick={() => setFileScope("all")}
              aria-pressed={fileScope === "all"}
              class="px-2.5 py-2 text-[11px] font-semibold transition-colors {fileScope === 'all' ? 'bg-accent-subtle text-accent' : 'bg-surface-bg text-text-muted hover:text-text-primary'}"
            >
              Whole scan
            </button>
            <button
              onclick={() => setFileScope("folder")}
              aria-pressed={fileScope === "folder"}
              disabled={!focusNode}
              title={focusNode ? `Limit to ${focusNode.path}` : "Open a folder in the treemap first"}
              class="px-2.5 py-2 text-[11px] font-semibold border-l border-border-default transition-colors disabled:opacity-40 {fileScope === 'folder' ? 'bg-accent-subtle text-accent' : 'bg-surface-bg text-text-muted hover:text-text-primary'}"
            >
              This folder
            </button>
          </div>
          <button
            onclick={exportTopFilesCsv}
            class="px-3 py-2 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-xs font-semibold text-text-secondary hover:text-text-primary transition-colors"
          >
            Export CSV
          </button>
          <span class="text-[11px] font-mono text-text-muted self-center" aria-live="polite">
            {#if isSearching}
              Searching…
            {:else if isSearchActive && searchResult}
              {searchResult.total_matches.toLocaleString()} matches · {searchResult.indexed_files.toLocaleString()} indexed{searchResult.index_complete ? "" : " (capped)"}
            {:else}
              Top {displayFiles.length} by size · search covers all scanned files
            {/if}
          </span>
        </div>

        <div class="border border-border-default rounded-lg overflow-hidden">
          <div class="overflow-x-auto">
            <table class="w-full text-left text-xs min-w-[620px]">
              <thead class="bg-sidebar-bg text-text-muted uppercase tracking-wider font-mono sticky top-0 z-10">
                <tr>
                  <th class="px-4 py-3">File</th>
                  <th class="px-4 py-3">Extension</th>
                  <th class="px-4 py-3 text-right">Allocated</th>
                  <th class="px-4 py-3 text-right">Logical</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-border-default">
                {#each displayFiles as file}
                  <tr class="hover:bg-elevated-bg/50 cursor-pointer" onclick={() => openFileFolder(file)}>
                    <td class="px-4 py-2.5">
                      <span class="text-text-primary font-mono break-all">{file.name}</span>
                      <span class="block text-[11px] text-text-muted font-mono truncate max-w-[560px]" title={file.path}>{file.path}</span>
                    </td>
                    <td class="px-4 py-2.5 font-mono text-text-secondary">{file.extension}</td>
                    <td class="px-4 py-2.5 text-right font-mono text-text-primary">{formatBytes(file.allocated_bytes)}</td>
                    <td class="px-4 py-2.5 text-right font-mono text-text-muted">{formatBytes(file.size_bytes)}</td>
                  </tr>
                {:else}
                  <tr>
                    <td colspan="4" class="px-4 py-8 text-center text-text-muted">
                      {isSearching ? "Searching indexed files…" : isSearchActive ? "No scanned files match this search." : "No top files match this filter."}
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
              title="Show all .{type.extension || '(none)'} files by size"
              class="w-full text-left p-2 rounded-md hover:bg-elevated-bg/70 focus-visible:bg-elevated-bg/70 transition-colors"
            >
              <div class="flex items-center justify-between text-xs font-mono gap-3">
                <span class="flex items-center gap-2 text-text-primary">
                  <span class="w-2.5 h-2.5 rounded-sm" style="background: {colorForExtension(type.extension)}"></span>
                  {type.extension || "(none)"}
                </span>
                <span class="text-text-muted">
                  {formatBytes(type.size_bytes)} · {type.file_count.toLocaleString()} files ·
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
            Enable the <b class="text-text-secondary">Duplicates</b> option and scan again to compare files by name + size.
          </div>
        {:else if result.duplicate_groups.length === 0}
          <div class="p-6 border border-dashed border-border-default rounded-lg bg-surface-bg/20 text-center text-xs text-text-muted">
            No duplicate groups found (same name + size).
          </div>
        {:else}
          <div class="space-y-3">
            {#each result.duplicate_groups as group}
              <div class="instrument-panel p-3.5 space-y-2">
                <div class="flex flex-wrap items-center justify-between gap-2">
                  <div class="flex items-center gap-2 min-w-0">
                    <File class="w-4 h-4 text-text-muted flex-shrink-0" />
                    <span class="text-sm font-semibold text-text-primary truncate" title={group.name}>{group.name}</span>
                    <span class="text-[10px] font-mono text-text-muted">{group.file_count} copies</span>
                  </div>
                  <span class="readout text-xs text-data-cyan">
                    {formatBytes(group.size_bytes)} each · {formatBytes(group.size_bytes * (group.file_count - 1))} waste
                  </span>
                </div>
                <div class="space-y-1">
                  {#each group.files as file}
                    <button
                      onclick={() => openFileFolder(file)}
                      class="w-full text-left px-2 py-1.5 rounded bg-app-bg/60 hover:bg-elevated-bg text-[11px] font-mono text-text-muted hover:text-text-primary transition-colors break-all"
                      title={file.path}
                    >
                      {file.path}
                    </button>
                  {/each}
                  {#if group.file_count > group.files.length}
                    <span class="block text-[10px] text-text-muted px-2">+{group.file_count - group.files.length} more copies</span>
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
          <h3 class="text-sm font-bold text-text-primary">Scan a drive or folder</h3>
          <p class="text-xs text-text-muted mt-1 max-w-md leading-relaxed">
            PurgeKit reads the selected tree once, shows allocated size (sparse/compressed aware),
            and lets you drill into the largest folders and files. Symbols, junctions and reparse points are skipped.
          </p>
        </div>
        <div class="flex items-center gap-2 text-[10px] text-text-muted font-mono">
          <File class="w-3.5 h-3.5" /> top files
          <Folder class="w-3.5 h-3.5" /> treemap
          <HardDrive class="w-3.5 h-3.5" /> allocated size
        </div>
      </div>
    {/if}
  </div>
</div>
