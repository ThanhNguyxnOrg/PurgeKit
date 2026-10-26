<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { FolderOpen, Play, RefreshCw, Square, HardDrive, File, Folder } from "@lucide/svelte";
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

  interface DiskScanResult {
    root_path: string;
    root: DiskTreeNode;
    top_files: DiskFile[];
    file_types: FileTypeStat[];
    total_files: number;
    total_dirs: number;
    total_size: number;
    total_allocated: number;
    elapsed_ms: number;
    errors: number;
    truncated: boolean;
  }

  interface DiskProgress {
    current_path: string;
    files: number;
    dirs: number;
    bytes: number;
  }

  interface LayoutRect {
    node: DiskTreeNode;
    x: number;
    y: number;
    w: number;
    h: number;
  }

  let pathInput = $state("C:\\");
  let result = $state<DiskScanResult | null>(null);
  let focusNode = $state<DiskTreeNode | null>(null);
  let focusStack = $state<DiskTreeNode[]>([]);
  let selectedNode = $state<DiskTreeNode | null>(null);
  let activeView = $state<"treemap" | "files" | "types">("treemap");
  let isScanning = $state(false);
  let isCancelling = $state(false);
  let progress = $state<DiskProgress | null>(null);
  let errorMessage = $state("");
  let canvasEl = $state<HTMLCanvasElement | null>(null);
  let layoutRects: LayoutRect[] = [];

  function formatBytes(bytes: number): string {
    if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    const index = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
    return `${(bytes / Math.pow(1024, index)).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
  }

  function nodeWeight(node: DiskTreeNode): number {
    return Math.max(node.allocated_bytes, node.size_bytes);
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

    try {
      const scanResult = await invoke<DiskScanResult>("scan_disk_space", {
        path: target,
        maxDepth: 4,
        maxChildren: 40,
      });
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
    const hue = (hash + index * 29) % 360;
    return node.is_dir
      ? `hsl(${hue} 45% 32%)`
      : `hsl(${hue} 38% 24%)`;
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
      context.fillStyle = colorForNode(rect.node, index);
      context.fillRect(rect.x, rect.y, rect.w, rect.h);

      context.strokeStyle = "rgba(9, 11, 15, 0.85)";
      context.lineWidth = 1;
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
    const hit = layoutRects.find(
      (rect) => x >= rect.x && x <= rect.x + rect.w && y >= rect.y && y <= rect.y + rect.h
    );
    if (!hit) return;

    selectedNode = hit.node;
    if (hit.node.is_dir && hit.node.children.length > 0) {
      focusStack = [...focusStack, hit.node];
      focusNode = hit.node;
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
    if (canvasEl && activeView === "treemap") {
      requestAnimationFrame(drawTreemap);
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
    const unlisten = listen<DiskProgress>("disk-scan-progress", (event) => {
      progress = event.payload;
    });

    const onResize = () => {
      if (activeView === "treemap") drawTreemap();
    };
    window.addEventListener("resize", onResize);

    return () => {
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

    <div class="flex flex-col md:flex-row gap-3 md:items-center">
      <div class="relative flex-1">
        <input
          bind:value={pathInput}
          onkeydown={(event) => event.key === "Enter" && !isScanning && scanDisk()}
          placeholder="C:\ or D:\Code"
          aria-label="Drive or folder to scan"
          class="w-full px-3 py-2 rounded-lg bg-surface-bg border border-border-default focus:border-accent/50 focus:ring-1 focus:ring-accent/30 outline-none text-sm font-mono text-text-primary placeholder:text-text-muted"
        />
      </div>
      <button
        onclick={() => { pathInput = "C:\\"; scanDisk(); }}
        disabled={isScanning}
        class="px-3 py-2 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-xs font-semibold text-text-secondary disabled:opacity-40"
      >
        C:\
      </button>
      <button
        onclick={scanDisk}
        disabled={isScanning || !pathInput.trim()}
        class="px-3 py-2 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-xs font-semibold text-text-secondary disabled:opacity-40"
      >
        <RefreshCw class="w-3.5 h-3.5 inline mr-1" />
        Scan path
      </button>
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
      <div class="grid grid-cols-2 lg:grid-cols-4 gap-3">
        <div class="p-4 rounded-lg bg-surface-bg/60 border border-border-default">
          <span class="text-[10px] uppercase tracking-wider text-text-muted font-mono">Allocated</span>
          <p class="text-lg font-bold text-text-primary font-mono mt-1">{formatBytes(result.total_allocated)}</p>
        </div>
        <div class="p-4 rounded-lg bg-surface-bg/60 border border-border-default">
          <span class="text-[10px] uppercase tracking-wider text-text-muted font-mono">Logical</span>
          <p class="text-lg font-bold text-text-primary font-mono mt-1">{formatBytes(result.total_size)}</p>
        </div>
        <div class="p-4 rounded-lg bg-surface-bg/60 border border-border-default">
          <span class="text-[10px] uppercase tracking-wider text-text-muted font-mono">Files / Folders</span>
          <p class="text-lg font-bold text-text-primary font-mono mt-1">
            {result.total_files.toLocaleString()} / {result.total_dirs.toLocaleString()}
          </p>
        </div>
        <div class="p-4 rounded-lg bg-surface-bg/60 border border-border-default">
          <span class="text-[10px] uppercase tracking-wider text-text-muted font-mono">Scan time</span>
          <p class="text-lg font-bold text-text-primary font-mono mt-1">{(result.elapsed_ms / 1000).toFixed(2)}s</p>
          {#if result.errors > 0}
            <p class="text-[10px] text-amber-300 mt-1">{result.errors} entries skipped</p>
          {/if}
        </div>
      </div>

      {#if result.truncated}
        <div class="p-3 rounded-lg bg-amber-950/20 border border-amber-900/40 text-xs text-amber-300">
          The tree is truncated to keep the UI responsive. Largest files and file types still cover the full scan.
        </div>
      {/if}

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
        <div class="flex-1"></div>
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

        <div class="relative w-full h-[520px] border border-border-default rounded-lg overflow-hidden bg-app-bg">
          <canvas bind:this={canvasEl} onclick={handleCanvasClick} class="block w-full h-full cursor-crosshair"></canvas>
        </div>

        <div class="flex flex-wrap items-center justify-between gap-3 text-xs">
          <div class="flex items-center gap-2 text-text-muted font-mono">
            <Folder class="w-3.5 h-3.5 text-accent" />
            <span class="break-all">{selectedNode ? selectedNode.path : (focusNode ? focusNode.path : "")}</span>
          </div>
          {#if selectedNode}
            <span class="font-mono text-text-secondary">
              {formatBytes(nodeWeight(selectedNode))} · {selectedNode.file_count.toLocaleString()} files
            </span>
          {/if}
        </div>
      {:else if activeView === "files"}
        <div class="border border-border-default rounded-lg overflow-hidden">
          <div class="overflow-x-auto">
            <table class="w-full text-left text-xs min-w-[620px]">
              <thead class="bg-sidebar-bg text-text-muted uppercase tracking-wider font-mono">
                <tr>
                  <th class="px-4 py-3">File</th>
                  <th class="px-4 py-3">Extension</th>
                  <th class="px-4 py-3 text-right">Allocated</th>
                  <th class="px-4 py-3 text-right">Logical</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-border-default">
                {#each result.top_files as file}
                  <tr class="hover:bg-elevated-bg/50 cursor-pointer" onclick={() => openFileFolder(file)}>
                    <td class="px-4 py-2.5">
                      <span class="text-text-primary font-mono break-all">{file.name}</span>
                      <span class="block text-[10px] text-text-muted font-mono truncate max-w-[560px]" title={file.path}>{file.path}</span>
                    </td>
                    <td class="px-4 py-2.5 font-mono text-text-secondary">{file.extension}</td>
                    <td class="px-4 py-2.5 text-right font-mono text-text-primary">{formatBytes(file.allocated_bytes)}</td>
                    <td class="px-4 py-2.5 text-right font-mono text-text-muted">{formatBytes(file.size_bytes)}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>
      {:else}
        <div class="border border-border-default rounded-lg bg-surface-bg/40 p-4 space-y-3">
          {#each result.file_types as type}
            <div>
              <div class="flex items-center justify-between text-xs font-mono">
                <span class="text-text-primary">{type.extension || "(none)"}</span>
                <span class="text-text-muted">
                  {formatBytes(type.size_bytes)} · {type.file_count.toLocaleString()} files ·
                  {result.total_allocated > 0 ? ((type.size_bytes / result.total_allocated) * 100).toFixed(1) : "0.0"}%
                </span>
              </div>
              <div class="h-2 bg-app-bg rounded-full overflow-hidden mt-1">
                <div
                  class="h-full bg-accent/70 rounded-full"
                  style="width: {result.total_allocated > 0 ? Math.max(1, (type.size_bytes / result.total_allocated) * 100) : 0}%"
                ></div>
              </div>
            </div>
          {/each}
        </div>
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
