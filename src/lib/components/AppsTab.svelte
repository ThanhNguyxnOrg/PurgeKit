<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { Search, RefreshCw, Trash2, X, CheckSquare, Square, Folder, Database, File } from "@lucide/svelte";
  import { toast } from "../toast.svelte";

  type AppSource = "registry" | "store" | "cli" | "toolchain" | "portable";

  interface InstalledApp {
    id: string;
    display_name: string;
    display_version: string | null;
    publisher: string | null;
    uninstall_string: string | null;
    quiet_uninstall_string: string | null;
    install_location: string | null;
    install_date: string | null;
    display_icon: string | null;
    estimated_size: number | null;
    registry_path: string;
    hive: string;
    is_uwp: boolean;
    icon_base64?: string | null;
    is_verified?: boolean | null;
    source?: AppSource;
    categories?: string[];
    tags?: string[];
    is_dev?: boolean;
    managed_by?: string | null;
    is_active?: boolean | null;
    is_portable?: boolean;
    is_uninstallable?: boolean;
    requires_admin?: boolean;
  }

  interface RemnantItem {
    path: string;
    item_type: string; // "File" | "Directory" | "RegistryKey"
    size: number;
    confidence: string;
    score: number;
  }


  let apps = $state<InstalledApp[]>([]);
  let searchQuery = $state("");
  let filterType = $state<"all" | "desktop" | "store" | "cli" | "toolchain" | "portable">("all");
  let devOnly = $state(false);
  let categoryFilter = $state("all");
  let sortBy = $state<"name" | "size" | "source">("name");
  let isLoading = $state(true);
  let statusMessage = $state("Scanning applications, Store packages & developer tools...");

  // Uninstall preflight: confirm exactly what will run and what is backed up.
  let preflightApp = $state<InstalledApp | null>(null);
  let preflightSettings = $state<{
    backup_before_delete: boolean;
    create_restore_point: boolean;
  } | null>(null);
  let isPreflightLoading = $state(false);
  let isForceRemoving = $state(false);

  function appSource(app: InstalledApp): AppSource {
    return app.source ?? (app.is_uwp ? "store" : "registry");
  }

  function canBulkUninstall(app: InstalledApp): boolean {
    const source = appSource(app);
    return source === "store" || (source === "registry" && !!app.uninstall_string);
  }

  // Remnants Modal States
  let activeApp = $state<InstalledApp | null>(null);
  let remnants = $state<RemnantItem[]>([]);
  let selectedRemnants = $state<Record<string, boolean>>({});
  let isScanningRemnants = $state(false);
  let isPurging = $state(false);
  let purgeResult = $state<{ success: number; fail: number } | null>(null);

  // App Checkbox Selection States
  let selectedApps = $state<Record<string, boolean>>({});
  let selectedAppsCount = $derived(Object.values(selectedApps).filter(Boolean).length);

  function toggleSelectApp(app: InstalledApp) {
    if (!canBulkUninstall(app)) return;
    selectedApps[app.id] = !selectedApps[app.id];
  }

  function toggleSelectAllApps(checked: boolean) {
    // Replace the selection instead of merging: select-all must never leave
    // hidden rows selected from a previous filter. CLI/toolchain entries are
    // individual-only and never enter the bulk silent-uninstall queue.
    if (checked) {
      selectedApps = Object.fromEntries(
        filteredApps.filter(canBulkUninstall).map(app => [app.id, true])
      );
    } else {
      selectedApps = {};
    }
  }

  function clearAppSelection() {
    selectedApps = {};
  }

  // Bulk Uninstall States
  let isBulkUninstalling = $state(false);
  let bulkUninstallStep = $state<"idle" | "running" | "remnants_list" | "done" | "error">("idle");
  let bulkUninstallLog = $state<string[]>([]);
  let bulkUninstallError = $state("");
  // Auto-purge is destructive and opt-in. Users must review leftovers first.
  let bulkAutoPurge = $state(false);
  let bulkRemnants = $state<RemnantItem[]>([]);
  let selectedBulkRemnants = $state<Record<string, boolean>>({});
  let bulkUninstallCurrent = $state(0);
  let bulkUninstallTotal = $state(0);
  let bulkUninstallCurrentAppName = $state("");
  let bulkUninstallStatusMessage = $state("");

  // Fetch apps from Tauri backend
  async function loadApps() {
    isLoading = true;
    try {
      apps = await invoke<InstalledApp[]>("get_installed_apps");
    } catch (e) {
      console.error("Failed to load apps:", e);
      toast.show("Error scanning application list!", "error");
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    loadApps();

    const unlistenScan = listen<{ phase: string; current: number; total: number; message: string }>(
      "scan-progress",
      (event) => {
        statusMessage = event.payload.message;
      }
    );

    const unlistenRemnant = listen<{ phase: string; current: number; total: number; message: string }>(
      "remnant-scan-progress",
      (event) => {
        if (uninstallStep === "scanning") {
          uninstallStatusText = event.payload.message;
        }
      }
    );

    const unlistenPurge = listen<{ phase: string; current: number; total: number; message: string }>(
      "purge-progress",
      (event) => {
        if (isPurging) {
          uninstallStatusText = event.payload.message;
        }
      }
    );

    const unlistenBulk = listen<{ app_id: string; app_name: string; phase: string; current: number; total: number; message: string }>(
      "bulk-uninstall-progress",
      (event) => {
        const payload = event.payload;
        bulkUninstallCurrent = payload.current;
        bulkUninstallTotal = payload.total;
        bulkUninstallCurrentAppName = payload.app_name;
        bulkUninstallStatusMessage = payload.message;
        
        const timestamp = new Date().toLocaleTimeString();
        bulkUninstallLog = [...bulkUninstallLog, `[${timestamp}] [${payload.phase.toUpperCase()}] ${payload.message}`];
      }
    );

    return () => {
      unlistenScan.then((fn) => fn());
      unlistenRemnant.then((fn) => fn());
      unlistenPurge.then((fn) => fn());
      unlistenBulk.then((fn) => fn());
    };
  });

  let availableCategories = $derived(
    Array.from(new Set(apps.flatMap((app) => app.categories ?? []))).sort()
  );

  // Filter and search logic
  let filteredApps = $derived(
    apps
      .filter((app) => {
        const source = appSource(app);
        const query = searchQuery.toLowerCase();
        const matchesSearch =
          app.display_name.toLowerCase().includes(query) ||
          (app.publisher && app.publisher.toLowerCase().includes(query)) ||
          (app.install_location && app.install_location.toLowerCase().includes(query)) ||
          (app.categories ?? []).some((category) => category.toLowerCase().includes(query)) ||
          (app.tags ?? []).some((tag) => tag.toLowerCase().includes(query));

        const matchesFilter =
          filterType === "all" ||
          (filterType === "desktop" && (source === "registry" || source === "portable")) ||
          filterType === source;

        const matchesDev = !devOnly || app.is_dev === true;
        const matchesCategory =
          categoryFilter === "all" || (app.categories ?? []).includes(categoryFilter);

        return matchesSearch && matchesFilter && matchesDev && matchesCategory;
      })
      .sort((left, right) => {
        if (sortBy === "size") {
          return (right.estimated_size ?? 0) - (left.estimated_size ?? 0);
        }
        if (sortBy === "source") {
          return appSource(left).localeCompare(appSource(right)) ||
            left.display_name.localeCompare(right.display_name);
        }
        return left.display_name.localeCompare(right.display_name);
      })
  );

  let selectedVisibleAppsCount = $derived(
    filteredApps.filter((app) => canBulkUninstall(app) && !!selectedApps[app.id]).length
  );
  let bulkableVisibleCount = $derived(filteredApps.filter(canBulkUninstall).length);

  let remnantGroups = $derived(
    [
      { type: "Directory", label: "Folders" },
      { type: "File", label: "Files" },
      { type: "RegistryKey", label: "Registry keys" },
      { type: "RegistryValue", label: "Registry values" },
      { type: "ScheduledTask", label: "Scheduled tasks" },
    ]
      .map((group) => ({
        ...group,
        items: remnants.filter((item) => item.item_type === group.type),
      }))
      .filter((group) => group.items.length > 0)
  );

  function selectGroup(type: string, selected: boolean) {
    remnants
      .filter((item) => item.item_type === type)
      .forEach((item) => {
        selectedRemnants[item.path] = selected;
      });
  }

  function remnantReason(item: RemnantItem): string {
    if (item.item_type === "ScheduledTask") return "Task references this app or its install path";
    if (item.item_type === "RegistryKey" || item.item_type === "RegistryValue") {
      return item.path.toLowerCase().includes("\\uninstall\\")
        ? "Registered uninstall entry"
        : "Name/path-matched registry trace";
    }
    return item.confidence === "VeryHigh"
      ? "Strong install-location match"
      : "Name-matched leftover in a scanned location";
  }

  // Helper to format file size
  function formatSize(bytes: number | null): string {
    if (bytes === null || bytes <= 0 || isNaN(bytes)) return "Unknown";
    const k = 1024;
    const sizes = ["Bytes", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  // Helper to generate consistent avatar background and text color based on name
  function getAvatarStyle(name: string): string {
    const charCode = name.charCodeAt(0) || 0;
    const colors = [
      "bg-emerald-950/40 text-emerald-400 border-emerald-800/30",
      "bg-blue-950/40 text-blue-400 border-blue-800/30",
      "bg-brand-purple/10 text-brand-purple border-brand-purple/20",
      "bg-amber-950/40 text-amber-400 border-amber-800/30",
      "bg-rose-950/40 text-rose-400 border-rose-800/30",
    ];
    return colors[charCode % colors.length];
  }

  let uninstallStep = $state("idle");
  let uninstallStatusText = $state("");

  // Handle standard uninstall and leftovers scanning flow
  async function startUninstallFlow(app: InstalledApp) {
    activeApp = app;
    remnants = [];
    selectedRemnants = {};
    purgeResult = null;
    isScanningRemnants = false;
    
    // Step 1: Run official uninstaller (if present)
    if (app.uninstall_string) {
      uninstallStep = "uninstalling";
      uninstallStatusText = "Executing the official uninstaller. Please follow the uninstallation GUI prompts on your screen...";
      try {
        await invoke("run_uninstall_command", { uninstallString: app.uninstall_string });
      } catch (e: any) {
        console.error("Official uninstaller failed or was closed:", e);
        toast.show("The official uninstaller returned an error or was closed.", "warning");
      }
    }

    // Step 2: Scan for remnants
    uninstallStep = "scanning";
    uninstallStatusText = "Scanning system directories and registry paths for leftover traces...";
    isScanningRemnants = true;
    
    try {
      remnants = await invoke<RemnantItem[]>("get_app_remnants", {
        appName: app.display_name,
        publisher: app.publisher,
        installLocation: app.install_location,
      });

      // Only auto-select the strongest evidence. High/Medium/Low items remain
      // visible but require an explicit user choice.
      remnants.forEach((item) => {
        selectedRemnants[item.path] = item.confidence === "VeryHigh";
      });
    } catch (e) {
      console.error("Failed to scan remnants:", e);
      toast.show("Error scanning remnant files on the system!", "error");
    } finally {
      isScanningRemnants = false;
      uninstallStep = "remnants_list";
    }
  }

  async function startCliUninstall(app: InstalledApp, confirmed = false) {
    const manager = app.managed_by;
    if (!manager) return;

    if (!confirmed && !confirm(`Uninstall global ${manager} package "${app.display_name}"?`)) {
      return;
    }

    activeApp = app;
    remnants = [];
    selectedRemnants = {};
    purgeResult = null;
    isScanningRemnants = true;
    uninstallStep = "uninstalling";
    uninstallStatusText = `Uninstalling ${app.display_name} via ${manager}...`;

    try {
      const binNames = app.install_location
        ? await invoke<string[]>("get_cli_package_bin_names", { packagePath: app.install_location })
        : [];

      await invoke("uninstall_global_cli_package", {
        name: app.display_name,
        manager,
      });

      uninstallStep = "scanning";
      uninstallStatusText = `Scanning package-owned paths for ${app.display_name}...`;

      const found = await invoke<RemnantItem[]>("get_cli_package_remnants", {
        name: app.display_name,
        manager,
        packagePath: app.install_location || "",
        binNames,
      });

      remnants = found;
      found.forEach((item) => {
        selectedRemnants[item.path] = item.confidence === "VeryHigh";
      });
      uninstallStep = "remnants_list";
    } catch (e: any) {
      toast.show(`Package uninstall failed: ${e.toString()}`, "error");
      closeRemnantsModal();
    } finally {
      isScanningRemnants = false;
    }
  }

  async function startToolchainUninstall(app: InstalledApp, confirmed = false) {
    const manager = app.managed_by;
    const version = app.display_version;
    if (!manager || !version || !app.install_location) return;

    const warning = app.is_active
      ? `\n\nWARNING: ${version} is the ACTIVE ${manager} toolchain. Uninstalling it can break existing projects.`
      : "";
    if (!confirmed && !confirm(`Uninstall ${manager} toolchain "${version}"?${warning}`)) {
      return;
    }

    try {
      await invoke("delete_toolchain_version", {
        manager,
        version,
        path: app.install_location,
      });
      toast.show(`Uninstalled ${manager} ${version}.`, "success");
      await loadApps();
    } catch (e: any) {
      toast.show(`Toolchain uninstall failed: ${e.toString()}`, "error");
    }
  }

  async function openPreflight(app: InstalledApp) {
    preflightApp = app;
    preflightSettings = null;
    isPreflightLoading = true;
    try {
      const settings = await invoke<{
        backup_before_delete: boolean;
        create_restore_point: boolean;
      }>("get_settings");
      preflightSettings = {
        backup_before_delete: settings.backup_before_delete,
        create_restore_point: settings.create_restore_point,
      };
    } catch (e) {
      console.error("Failed to load uninstall settings:", e);
    } finally {
      isPreflightLoading = false;
    }
  }

  function closePreflight() {
    if (isForceRemoving) return;
    preflightApp = null;
    preflightSettings = null;
  }

  async function confirmPreflight() {
    const app = preflightApp;
    if (!app) return;

    const source = appSource(app);

    if (source === "portable") {
      await forceRemovePreflight(true);
      return;
    }

    closePreflight();

    if (source === "cli") {
      await startCliUninstall(app, true);
      return;
    }
    if (source === "toolchain") {
      await startToolchainUninstall(app, true);
      return;
    }

    await startUninstallFlow(app);
  }

  async function forceRemovePreflight(confirmed = false) {
    const app = preflightApp;
    if (!app) return;

    const source = appSource(app);
    if (!confirmed) {
      const confirmedByUser = confirm(
        `FORCE REMOVE "${app.display_name}"?\n\n` +
        `This bypasses the official uninstaller and removes:\n` +
        `- install folder: ${app.install_location || "(none)"}\n` +
        `- uninstall registry entry: ${source === "registry" ? app.registry_path || "(none)" : "(not applicable)"}\n\n` +
        `Files are quarantined if backup is enabled; registry is exported first.`
      );
      if (!confirmedByUser) return;
    }

    isForceRemoving = true;
    try {
      const result = await invoke<{ success: number; fail: number; errors?: string[] }>(
        "force_remove_app",
        {
          installLocation: app.install_location,
          registryPath: source === "registry" ? app.registry_path || null : null,
          hive: source === "registry" ? app.hive || null : null,
        }
      );

      if (result.fail > 0) {
        toast.show(
          `Force remove finished with ${result.fail} failure(s): ${result.errors?.[0] || "see logs"}`,
          "warning"
        );
      } else {
        toast.show(`Force removed ${app.display_name}.`, "success");
      }
      isForceRemoving = false;
      preflightApp = null;
      preflightSettings = null;
      await loadApps();
    } catch (e: any) {
      toast.show(`Force remove failed: ${e.toString()}`, "error");
    } finally {
      isForceRemoving = false;
    }
  }

  async function removeUninstallEntry(app: InstalledApp) {
    const confirmed = confirm(
      `Remove the uninstall entry for "${app.display_name}"?\n\n` +
      `Registry key: ${app.registry_path}\n\n` +
      `This only removes the registry record. Files are NOT deleted.`
    );
    if (!confirmed) return;

    try {
      await invoke("remove_uninstall_entry", {
        hive: app.hive,
        registryPath: app.registry_path,
      });
      toast.show(`Removed the uninstall entry for ${app.display_name}.`, "success");
      await loadApps();
    } catch (e: any) {
      toast.show(`Failed to remove entry: ${e.toString()}`, "error");
    }
  }

  // Toggle selection for single item
  function toggleSelection(path: string) {
    selectedRemnants[path] = !selectedRemnants[path];
  }

  // Toggle select all
  function selectAll(val: boolean) {
    remnants.forEach((item) => {
      selectedRemnants[item.path] = val;
    });
  }

  // Perform purge remnants
  async function performPurge() {
    const itemsToPurge = remnants.filter((r) => selectedRemnants[r.path]);
    if (itemsToPurge.length === 0) {
      toast.show("Please select at least one remnant item to clean.", "warning");
      return;
    }

    isPurging = true;
    try {
      const result = await invoke<{ success: number; fail: number }>("purge_remnants", {
        items: itemsToPurge,
      });
      purgeResult = result;
      toast.show(`Purge completed! Successfully removed ${result.success} remnant items.`, "success");
      // Reload apps list
      await loadApps();
    } catch (e: any) {
      toast.show(`Error purging remnants: ${e.toString()}`, "error");
    } finally {
      isPurging = false;
    }
  }


  function closeRemnantsModal() {
    activeApp = null;
    remnants = [];
    selectedRemnants = {};
    purgeResult = null;
    uninstallStep = "idle";
    // The app may have been removed by the official uninstaller even if the
    // user closes the leftover review without purging anything.
    void loadApps();
  }

  function escapeHtml(value: string): string {
    return value.replace(/[&<>"']/g, (character) => ({
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      '"': "&quot;",
      "'": "&#39;",
    })[character] ?? character);
  }

  function exportApps(format: "csv" | "json" | "html") {
    if (filteredApps.length === 0) {
      toast.show("No application data to export!", "warning");
      return;
    }

    let content = "";
    let mimeType = "";
    let filename = "";

    if (format === "json") {
      content = JSON.stringify(filteredApps, null, 2);
      mimeType = "application/json";
      filename = `purgekit_apps_${new Date().toISOString().slice(0, 10)}.json`;
    } else if (format === "html") {
      const rows = filteredApps.map((app) => `
        <tr>
          <td>${escapeHtml(app.display_name)}</td>
          <td>${escapeHtml(app.publisher || "Unknown")}</td>
          <td>${escapeHtml(app.display_version || "—")}</td>
          <td>${formatSize(app.estimated_size)}</td>
          <td>${escapeHtml(appSource(app).toUpperCase())}</td>
          <td>${app.is_dev ? "DEV" : "—"}${app.categories?.length ? ` · ${escapeHtml(app.categories.join(", "))}` : ""}</td>
          <td>${app.is_verified === true ? "Verified" : app.is_verified === false ? "Unverified" : "Unknown"}</td>
          <td class="path">${escapeHtml(app.install_location || app.registry_path)}</td>
        </tr>`).join("");

      content = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>PurgeKit application report</title>
<style>
body{font:14px system-ui,Segoe UI,sans-serif;background:#0f1115;color:#e6e8eb;padding:24px}
h1{font-size:20px;margin:0 0 4px}p{color:#9aa4b2;margin:0 0 20px}
table{width:100%;border-collapse:collapse}th,td{text-align:left;padding:9px 10px;border-bottom:1px solid #2a2f3a}
th{color:#9aa4b2;font-size:11px;text-transform:uppercase;letter-spacing:.06em}
tr:hover td{background:#171b22}.path{font-family:ui-monospace,Consolas,monospace;font-size:11px;color:#9aa4b2}
</style></head><body>
<h1>PurgeKit application report</h1>
<p>${filteredApps.length} applications · generated ${new Date().toLocaleString()}</p>
<table><thead><tr><th>Name</th><th>Publisher</th><th>Version</th><th>Size</th><th>Source</th><th>Dev / Categories</th><th>Signature</th><th>Location</th></tr></thead>
<tbody>${rows}</tbody></table></body></html>`;
      mimeType = "text/html;charset=utf-8";
      filename = `purgekit_apps_${new Date().toISOString().slice(0, 10)}.html`;
    } else {
      const headers = ["ID", "Name", "Version", "Publisher", "Source", "IsDev", "Categories", "InstallLocation", "RegistryPath", "Hive", "IsStore", "IsVerified"];
      const rows = filteredApps.map((app) => [
        app.id,
        app.display_name,
        app.display_version || "",
        app.publisher || "",
        appSource(app),
        app.is_dev ? "TRUE" : "FALSE",
        (app.categories || []).join("; "),
        app.install_location || "",
        app.registry_path,
        app.hive,
        app.is_uwp ? "TRUE" : "FALSE",
        app.is_verified === true ? "TRUE" : app.is_verified === false ? "FALSE" : "UNKNOWN"
      ]);
      
      const csvContent = [
        headers.join(","),
        ...rows.map((row) => row.map((val) => `"${val.replace(/"/g, '""')}"`).join(","))
      ].join("\n");
      
      content = "\uFEFF" + csvContent;
      mimeType = "text/csv;charset=utf-8;";
      filename = `purgekit_apps_${new Date().toISOString().slice(0, 10)}.csv`;
    }

    const blob = new Blob([content], { type: mimeType });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.setAttribute("href", url);
    link.setAttribute("download", filename);
    link.style.visibility = "hidden";
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    
    toast.show(`Successfully exported report to ${format.toUpperCase()}!`, "success");
  }

  async function startBulkUninstall() {
    const visibleIds = new Set(filteredApps.map((app) => app.id));
    const selectedIds = Object.entries(selectedApps)
      .filter(([id, val]) => val && visibleIds.has(id))
      .map(([id]) => id);
    if (selectedIds.length === 0) return;

    isBulkUninstalling = true;
    bulkUninstallStep = "running";
    bulkUninstallError = "";
    bulkUninstallLog = [`Initializing bulk silent uninstallation for ${selectedIds.length} application(s)...`];
    bulkRemnants = [];
    selectedBulkRemnants = {};
    
    try {
      const remainingRemnants = await invoke<RemnantItem[]>("run_bulk_silent_uninstall", {
        appIds: selectedIds,
        autoPurge: bulkAutoPurge,
      });
      
      bulkRemnants = remainingRemnants;
      
      bulkRemnants.forEach((item) => {
        selectedBulkRemnants[item.path] = item.confidence === "VeryHigh";
      });
      
      if (bulkRemnants.length > 0) {
        bulkUninstallStep = "remnants_list";
      } else {
        bulkUninstallStep = "done";
        toast.show("Bulk uninstallation and auto-purge completed! No remaining leftovers found.", "success");
        clearAppSelection();
        await loadApps();
      }
    } catch (e: any) {
      console.error("Bulk silent uninstallation failed:", e);
      bulkUninstallError = e.toString();
      toast.show(`Bulk uninstallation error: ${e.toString()}`, "error");
      bulkUninstallStep = "error";
      clearAppSelection();
      await loadApps();
    }
  }

  async function performBulkRemnantsPurge() {
    const itemsToPurge = bulkRemnants.filter((r) => selectedBulkRemnants[r.path]);
    if (itemsToPurge.length === 0) {
      toast.show("No leftovers selected to clean. Closing.", "info");
      bulkUninstallStep = "done";
      clearAppSelection();
      await loadApps();
      return;
    }
    
    isPurging = true;
    try {
      const result = await invoke<{ success: number; fail: number }>("purge_remnants", {
        items: itemsToPurge,
      });
      toast.show(`Purge completed! Removed ${result.success} leftover items.`, "success");
      bulkUninstallStep = "done";
      clearAppSelection();
      await loadApps();
    } catch (e: any) {
      toast.show(`Error purging leftovers: ${e.toString()}`, "error");
    } finally {
      isPurging = false;
    }
  }

  function closeBulkModal() {
    isBulkUninstalling = false;
    bulkUninstallStep = "idle";
    bulkUninstallLog = [];
    bulkRemnants = [];
    selectedBulkRemnants = {};
    clearAppSelection();
    loadApps();
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;

    if (preflightApp && !isForceRemoving) {
      event.preventDefault();
      closePreflight();
      return;
    }

    if (
      activeApp &&
      !isPurging &&
      uninstallStep !== "uninstalling" &&
      uninstallStep !== "scanning"
    ) {
      event.preventDefault();
      closeRemnantsModal();
      return;
    }

    if (isBulkUninstalling && bulkUninstallStep !== "running") {
      event.preventDefault();
      closeBulkModal();
    }
  }
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<div class="flex-1 flex flex-col h-full bg-app-bg text-text-primary relative">
  <!-- Section Header -->
  <header class="p-6 border-b border-border-default flex items-center justify-between bg-sidebar-bg">
    <div>
      <h2 class="text-2xl font-bold font-sans text-text-primary">
        Apps Manager
      </h2>
      <p class="text-xs text-text-muted mt-1">Manage and clean standard desktop apps & Windows Store apps</p>
    </div>

    <div class="flex items-center gap-2">
      <button
        onclick={loadApps}
        disabled={isLoading}
        class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-semibold bg-surface-bg border border-border-default hover:bg-elevated-bg hover:border-border-strong disabled:opacity-50 transition-all text-text-primary cursor-pointer active:scale-95"
      >
        <RefreshCw class="w-3.5 h-3.5 {isLoading ? 'animate-spin' : ''}" />
        Reload List
      </button>

      <!-- Export Dropdown -->
      <div class="relative inline-block text-left group">
        <button
          class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-semibold bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-primary cursor-pointer active:scale-95"
        >
          <svg class="w-3.5 h-3.5 text-text-secondary" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
            <polyline points="7 10 12 15 17 10"/>
            <line x1="12" y1="15" x2="12" y2="3"/>
          </svg>
          Export
        </button>
        <div class="absolute right-0 mt-1.5 w-32 rounded-lg bg-surface-bg border border-border-default shadow-lg opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all duration-150 z-50 p-1 flex flex-col gap-0.5">
          <button
            onclick={() => exportApps("csv")}
            class="w-full text-left px-2.5 py-1.5 rounded-md hover:bg-elevated-bg text-xs text-text-secondary hover:text-text-primary transition-colors font-sans font-semibold cursor-pointer"
          >
            Export to CSV
          </button>
          <button
            onclick={() => exportApps("json")}
            class="w-full text-left px-2.5 py-1.5 rounded-md hover:bg-elevated-bg text-xs text-text-secondary hover:text-text-primary transition-colors font-sans font-semibold cursor-pointer"
          >
            Export to JSON
          </button>
          <button
            onclick={() => exportApps("html")}
            class="w-full text-left px-2.5 py-1.5 rounded-md hover:bg-elevated-bg text-xs text-text-secondary hover:text-text-primary transition-colors font-sans font-semibold cursor-pointer"
          >
            Export to HTML
          </button>
        </div>
      </div>
    </div>
  </header>

  <!-- Filter Toolbar -->
  <div class="p-6 pb-2 flex flex-col md:flex-row gap-4 justify-between items-stretch">
    <!-- Search Bar -->
    <div class="relative flex-1 max-w-md">
      <Search class="absolute left-3.5 top-1/2 -translate-y-1/2 w-4 h-4 text-text-muted" />
      <input
        type="text"
        placeholder="Search apps by name or publisher..."
        bind:value={searchQuery}
        class="w-full pl-10 pr-4 py-2 rounded-lg bg-surface-bg border border-border-default focus:border-accent/50 focus:ring-1 focus:ring-accent/30 outline-none text-sm font-sans transition-all text-text-primary placeholder:text-text-muted"
      />
    </div>

    <!-- Inventory Filters -->
    <div class="flex flex-wrap items-center gap-2 self-start md:self-auto">
      <select
        bind:value={filterType}
        aria-label="Filter by source"
        class="px-3 py-2 rounded-lg bg-surface-bg border border-border-default text-xs font-semibold text-text-secondary hover:text-text-primary outline-none cursor-pointer"
      >
        <option value="all">All sources ({apps.length})</option>
        <option value="desktop">Desktop ({apps.filter(a => appSource(a) === 'registry' || appSource(a) === 'portable').length})</option>
        <option value="store">Store ({apps.filter(a => appSource(a) === 'store').length})</option>
        <option value="cli">CLI packages ({apps.filter(a => appSource(a) === 'cli').length})</option>
        <option value="toolchain">Toolchains ({apps.filter(a => appSource(a) === 'toolchain').length})</option>
        <option value="portable">Portable ({apps.filter(a => appSource(a) === 'portable').length})</option>
      </select>

      <label class="flex items-center gap-2 px-3 py-2 rounded-lg bg-surface-bg border border-border-default text-xs font-semibold text-text-secondary cursor-pointer select-none">
        <input type="checkbox" bind:checked={devOnly} class="rounded border-border-default text-accent focus:ring-accent bg-surface-bg cursor-pointer w-3.5 h-3.5" />
        Dev only
      </label>

      {#if availableCategories.length > 0}
        <select
          bind:value={categoryFilter}
          aria-label="Filter by developer category"
          class="px-3 py-2 rounded-lg bg-surface-bg border border-border-default text-xs font-semibold text-text-secondary hover:text-text-primary outline-none cursor-pointer"
        >
          <option value="all">All categories</option>
          {#each availableCategories as category}
            <option value={category}>{category}</option>
          {/each}
        </select>
      {/if}

      <select
        bind:value={sortBy}
        aria-label="Sort applications"
        class="px-3 py-2 rounded-lg bg-surface-bg border border-border-default text-xs font-semibold text-text-secondary hover:text-text-primary outline-none cursor-pointer"
      >
        <option value="name">Sort: Name</option>
        <option value="size">Sort: Size</option>
        <option value="source">Sort: Source</option>
      </select>
    </div>
  </div>

  <!-- Content Area -->
  <div class="flex-1 overflow-y-auto px-6 pb-6">
    {#if isLoading}
      <div class="flex flex-col items-center justify-center h-64 gap-3">
        <RefreshCw class="w-8 h-8 text-accent animate-spin" />
        <span class="text-sm text-text-muted font-mono">{statusMessage}</span>
      </div>

    {:else if filteredApps.length === 0}
      <div class="flex flex-col items-center justify-center h-64 border border-dashed border-border-default rounded-lg bg-surface-bg/30">
        <span class="text-sm text-text-muted">No applications matched your search criteria.</span>
      </div>
    {:else}
      <div class="border border-border-default rounded-lg overflow-hidden bg-surface-bg/50">
        <div class="overflow-x-auto w-full">
          <table class="w-full text-left border-collapse min-w-[800px]">
            <thead>
              <tr class="bg-sidebar-bg border-b border-border-default text-xs font-semibold text-text-secondary uppercase tracking-wider font-mono">
                <th class="py-4 px-5 w-10">
                  <input
                    type="checkbox"
                    aria-label="Select all applications"
                    checked={selectedVisibleAppsCount === bulkableVisibleCount && bulkableVisibleCount > 0}
                    onchange={(e) => toggleSelectAllApps(e.currentTarget.checked)}
                    class="rounded border-border-default text-accent focus:ring-accent bg-surface-bg cursor-pointer w-4 h-4"
                  />
                </th>
                <th class="py-4 px-5">App Name</th>
                <th class="py-4 px-5">Publisher</th>
                <th class="py-4 px-5">Version</th>
                <th class="py-4 px-5">Size</th>
                <th class="py-4 px-5">Source</th>
                <th class="py-4 px-5 text-right">Actions</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-border-default text-sm">
              {#each filteredApps as app}
                <tr class="hover:bg-elevated-bg/50 transition-colors duration-150 group {selectedApps[app.id] ? 'bg-accent/5' : ''}">
                  <td class="py-4 px-5 w-10">
                    <input
                      type="checkbox"
                      aria-label="Select application {app.display_name}"
                      checked={!!selectedApps[app.id]}
                      disabled={!canBulkUninstall(app)}
                      title={canBulkUninstall(app) ? "Select for bulk uninstall" : "CLI/toolchain entries are uninstalled individually"}
                      onchange={() => toggleSelectApp(app)}
                      class="rounded border-border-default text-accent focus:ring-accent bg-surface-bg cursor-pointer w-4 h-4 disabled:opacity-30 disabled:cursor-not-allowed"
                    />
                  </td>
                  <!-- Name -->
                  <td class="py-4 px-5 font-sans font-medium flex items-center gap-3">
                    <div class="w-8 h-8 rounded-lg border flex items-center justify-center overflow-hidden transition-colors shadow-sm bg-surface-bg border-border-default flex-shrink-0">
                      {#if app.icon_base64}
                        <img src="data:image/png;base64,{app.icon_base64}" class="w-6 h-6 object-contain" alt="" />
                      {:else}
                        <div class="w-full h-full flex items-center justify-center font-mono text-xs {getAvatarStyle(app.display_name)}">
                          {app.display_name.charAt(0).toUpperCase()}
                        </div>
                      {/if}
                    </div>
                    <div class="flex flex-col min-w-0">
                      <div class="flex items-center gap-1.5">
                        <span class="text-text-primary group-hover:text-white transition-colors max-w-[180px] md:max-w-[280px] truncate" title={app.display_name}>{app.display_name}</span>
                        {#if app.is_verified}
                          <span class="inline-flex items-center" title="Verified Publisher Certificate">
                            <svg class="w-3.5 h-3.5 text-emerald-400 fill-emerald-400/20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
                              <path d="m9 11 2 2 4-4"/>
                            </svg>
                          </span>
                        {/if}
                        {#if app.is_dev}
                          <span class="px-1 py-0.5 rounded text-[9px] font-mono font-bold bg-brand-purple/10 text-brand-purple border border-brand-purple/20" title={(app.categories ?? []).join(", ")}>DEV</span>
                        {/if}
                        {#if app.is_active}
                          <span class="px-1 py-0.5 rounded text-[9px] font-mono font-bold bg-success/10 text-success border border-success/20">ACTIVE</span>
                        {/if}
                      </div>
                      <span class="text-[10px] text-text-muted font-mono mt-0.5 max-w-[200px] truncate" title={app.registry_path}>
                        {app.id}
                      </span>
                      {#if app.categories && app.categories.length > 0}
                        <span class="text-[9px] text-text-muted font-sans mt-0.5 truncate" title={app.categories.join(", ")}>
                          {app.categories.slice(0, 2).join(" · ")}
                        </span>
                      {/if}
                    </div>
                  </td>
                  
                  <!-- Publisher -->
                  <td class="py-4 px-5 text-text-secondary max-w-[150px] truncate">
                    {app.publisher || "Unknown"}
                  </td>
                  
                  <!-- Version -->
                  <td class="py-4 px-5 text-text-secondary font-mono text-xs">
                    {app.display_version || "—"}
                  </td>
                  
                  <!-- Size -->
                  <td class="py-4 px-5 text-text-secondary font-mono text-xs">
                    {formatSize(app.estimated_size)}
                  </td>
                  
                  <!-- Hive / Type -->
                  <td class="py-4 px-5">
                    <span class="px-2 py-0.5 rounded text-[10px] font-mono border
                      {appSource(app) === 'store'
                        ? 'bg-blue-950/30 text-blue-400 border-blue-800/40'
                        : appSource(app) === 'cli'
                          ? 'bg-brand-purple/10 text-brand-purple border-brand-purple/20'
                          : appSource(app) === 'toolchain'
                            ? 'bg-emerald-950/30 text-emerald-400 border-emerald-800/40'
                            : appSource(app) === 'portable'
                              ? 'bg-amber-950/30 text-amber-400 border-amber-800/40'
                              : app.hive === 'HKCU'
                                ? 'bg-brand-purple/10 text-brand-purple border-brand-purple/20'
                                : 'bg-surface-bg text-text-secondary border-border-default'}">
                      {appSource(app) === 'registry' ? app.hive : appSource(app).toUpperCase()}
                    </span>
                  </td>
                  
                  <!-- Actions -->
                  <td class="py-4 px-5 text-right font-sans">
                    <div class="flex items-center justify-end gap-2">
                      {#if appSource(app) === "cli"}
                        <button
                          onclick={() => openPreflight(app)}
                          title="Uninstall this global package and review owned leftovers"
                          class="px-2.5 py-1.5 rounded-lg bg-brand-purple/10 hover:bg-brand-purple text-brand-purple hover:text-white transition-all border border-brand-purple/20 flex items-center gap-1.5 text-xs font-semibold active:scale-95 cursor-pointer"
                        >
                          <Trash2 class="w-3.5 h-3.5" />
                          Uninstall package
                        </button>
                      {:else if appSource(app) === "toolchain"}
                        <button
                          onclick={() => openPreflight(app)}
                          title="Uninstall this toolchain version"
                          class="px-2.5 py-1.5 rounded-lg bg-emerald-950/30 hover:bg-emerald-600 text-emerald-300 hover:text-white transition-all border border-emerald-800/40 flex items-center gap-1.5 text-xs font-semibold active:scale-95 cursor-pointer"
                        >
                          <Trash2 class="w-3.5 h-3.5" />
                          Uninstall
                        </button>
                      {:else if appSource(app) === "portable"}
                        <button
                          onclick={() => openPreflight(app)}
                          title="Remove this unregistered portable application folder"
                          class="px-2.5 py-1.5 rounded-lg bg-amber-950/30 hover:bg-amber-600 text-amber-300 hover:text-white transition-all border border-amber-800/40 flex items-center gap-1.5 text-xs font-semibold active:scale-95 cursor-pointer"
                        >
                          <Trash2 class="w-3.5 h-3.5" />
                          Remove folder
                        </button>
                      {:else if app.uninstall_string}
                        <button
                          onclick={() => openPreflight(app)}
                          title="Uninstall and Deep Clean Remnants"
                          class="px-2.5 py-1.5 rounded-lg bg-danger/10 hover:bg-danger text-danger hover:text-white transition-all border border-danger/20 flex items-center gap-1.5 text-xs font-semibold active:scale-95 cursor-pointer"
                        >
                          <Trash2 class="w-3.5 h-3.5" />
                          Uninstall
                        </button>
                      {:else}
                        <button
                          onclick={() => removeUninstallEntry(app)}
                          title="Remove only the orphaned uninstall registry entry"
                          class="px-2.5 py-1.5 rounded-lg bg-amber-950/30 hover:bg-amber-600 text-amber-300 hover:text-white transition-all border border-amber-800/40 flex items-center gap-1.5 text-xs font-semibold active:scale-95 cursor-pointer"
                        >
                          <Database class="w-3.5 h-3.5" />
                          Remove Entry
                        </button>
                      {/if}
                    </div>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
    {/if}
  </div>

  <!-- Uninstall Preflight Modal -->
  {#if preflightApp}
    <div
      class="fixed inset-0 z-50 bg-overlay-bg backdrop-blur-md flex items-center justify-center p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="preflight-title"
    >
      <div class="bg-surface-bg border border-border-default w-full max-w-xl rounded-xl flex flex-col max-h-[88vh] shadow-2xl relative overflow-hidden animate-scale-up duration-200">
        <header class="p-6 border-b border-border-default flex items-start justify-between gap-4">
          <div>
            <h3 id="preflight-title" class="text-base font-bold text-text-primary">Confirm uninstall</h3>
            <p class="text-xs text-text-muted mt-1">
              Review exactly what PurgeKit will run before anything is removed.
            </p>
          </div>
          <button
            onclick={closePreflight}
            disabled={isForceRemoving}
            class="p-1.5 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-colors disabled:opacity-40"
            aria-label="Close uninstall confirmation"
          >
            <X class="w-4 h-4" />
          </button>
        </header>

        <div class="flex-1 overflow-y-auto p-6 space-y-4">
          <div class="flex items-start gap-3">
            <div class="w-10 h-10 rounded-lg border border-border-default bg-app-bg flex items-center justify-center overflow-hidden flex-shrink-0">
              {#if preflightApp.icon_base64}
                <img src="data:image/png;base64,{preflightApp.icon_base64}" class="w-8 h-8 object-contain" alt="" />
              {:else}
                <span class="font-mono text-sm {getAvatarStyle(preflightApp.display_name)}">{preflightApp.display_name.charAt(0).toUpperCase()}</span>
              {/if}
            </div>
            <div class="min-w-0">
              <h4 class="text-sm font-bold text-text-primary break-all">{preflightApp.display_name}</h4>
              <div class="flex flex-wrap items-center gap-1.5 mt-1">
                <span class="px-1.5 py-0.5 rounded text-[9px] font-mono border border-border-default text-text-muted">
                  {appSource(preflightApp).toUpperCase()}
                </span>
                {#if preflightApp.is_dev}
                  <span class="px-1.5 py-0.5 rounded text-[9px] font-mono border border-brand-purple/20 bg-brand-purple/10 text-brand-purple">DEV</span>
                {/if}
                {#if preflightApp.is_active}
                  <span class="px-1.5 py-0.5 rounded text-[9px] font-mono border border-success/20 bg-success/10 text-success">ACTIVE</span>
                {/if}
              </div>
            </div>
          </div>

          <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 text-xs">
            <div class="p-3 rounded-lg bg-app-bg border border-border-default">
              <span class="block text-[10px] uppercase tracking-wider text-text-muted font-mono">Publisher</span>
              <span class="block mt-1 text-text-primary break-all">{preflightApp.publisher || "Unknown"}</span>
            </div>
            <div class="p-3 rounded-lg bg-app-bg border border-border-default">
              <span class="block text-[10px] uppercase tracking-wider text-text-muted font-mono">Version</span>
              <span class="block mt-1 text-text-primary font-mono break-all">{preflightApp.display_version || "—"}</span>
            </div>
            <div class="p-3 rounded-lg bg-app-bg border border-border-default">
              <span class="block text-[10px] uppercase tracking-wider text-text-muted font-mono">Estimated size</span>
              <span class="block mt-1 text-text-primary font-mono">{formatSize(preflightApp.estimated_size)}</span>
            </div>
            <div class="p-3 rounded-lg bg-app-bg border border-border-default">
              <span class="block text-[10px] uppercase tracking-wider text-text-muted font-mono">Managed by</span>
              <span class="block mt-1 text-text-primary font-mono break-all">
                {preflightApp.managed_by || (appSource(preflightApp) === "store" ? "Microsoft Store / Appx" : "Registry uninstall entry")}
              </span>
            </div>
          </div>

          <div class="p-3 rounded-lg bg-app-bg border border-border-default text-xs">
            <span class="block text-[10px] uppercase tracking-wider text-text-muted font-mono">Install location</span>
            <span class="block mt-1 text-text-primary font-mono break-all">{preflightApp.install_location || "Not registered"}</span>
          </div>

          <div class="p-3 rounded-lg bg-app-bg border border-border-default text-xs space-y-1.5">
            <span class="block text-[10px] uppercase tracking-wider text-text-muted font-mono">Execution method</span>
            {#if appSource(preflightApp) === "cli"}
              <p class="text-text-primary">Run <b>{preflightApp.managed_by}</b> package uninstall, then review package-owned leftovers.</p>
            {:else if appSource(preflightApp) === "toolchain"}
              <p class="text-text-primary">Run <b>{preflightApp.managed_by}</b> toolchain uninstall for <b>{preflightApp.display_version}</b>.</p>
            {:else if appSource(preflightApp) === "store"}
              <p class="text-text-primary">Run <b>Remove-AppxPackage</b> for the current user, then review leftovers.</p>
            {:else if appSource(preflightApp) === "portable"}
              <p class="text-text-primary">Move the portable application folder to quarantine. No registry uninstaller is available.</p>
            {:else}
              <p class="text-text-primary break-all">Run the registered uninstaller: <span class="font-mono">{preflightApp.uninstall_string || "no uninstall string"}</span></p>
            {/if}
            {#if preflightApp.requires_admin}
              <p class="text-amber-300">Administrator rights are required for this operation.</p>
            {/if}
            {#if preflightApp.is_active}
              <p class="text-amber-300">WARNING: this toolchain is currently ACTIVE. Removing it can break existing projects.</p>
            {/if}
          </div>

          <div class="p-3 rounded-lg bg-app-bg border border-border-default text-xs space-y-1.5">
            <span class="block text-[10px] uppercase tracking-wider text-text-muted font-mono">Safety settings</span>
            {#if isPreflightLoading}
              <p class="text-text-muted">Reading settings…</p>
            {:else if preflightSettings}
              <p class="text-text-primary">Backup/quarantine before delete: <b>{preflightSettings.backup_before_delete ? "On" : "OFF"}</b></p>
              <p class="text-text-primary">Create restore point: <b>{preflightSettings.create_restore_point ? "On" : "Off"}</b></p>
              {#if !preflightSettings.backup_before_delete}
                <p class="text-amber-300">Backup is off. Removed files may not be restorable.</p>
              {/if}
            {:else}
              <p class="text-text-muted">Could not read settings; backend defaults apply.</p>
            {/if}
          </div>

          {#if (appSource(preflightApp) === "registry" || appSource(preflightApp) === "portable") && preflightApp.install_location}
            <div class="p-3 rounded-lg bg-danger/5 border border-danger/20 text-xs space-y-2">
              <h4 class="font-bold text-danger">Force removal fallback</h4>
              <p class="text-text-muted leading-relaxed">
                Bypasses the official uninstaller and removes the install folder plus the uninstall registry entry.
                Use only when the normal uninstaller is missing or broken.
              </p>
              <button
                onclick={() => forceRemovePreflight()}
                disabled={isForceRemoving}
                class="px-3 py-1.5 rounded-lg bg-danger/10 hover:bg-danger text-danger hover:text-white border border-danger/20 text-xs font-semibold transition-all disabled:opacity-40"
              >
                {isForceRemoving ? "Force removing…" : "Force Remove"}
              </button>
            </div>
          {/if}
        </div>

        <footer class="p-6 border-t border-border-default flex justify-end items-center gap-3 bg-sidebar-bg">
          <button
            onclick={closePreflight}
            disabled={isForceRemoving}
            class="px-4 py-2 rounded-lg text-xs font-semibold bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-all disabled:opacity-40"
          >
            Cancel
          </button>
          <button
            onclick={confirmPreflight}
            disabled={isForceRemoving || isPreflightLoading}
            class="flex items-center gap-1.5 px-5 py-2 rounded-lg text-xs font-semibold bg-accent hover:bg-accent-hover text-white transition-all active:scale-95 disabled:opacity-50"
          >
            <Trash2 class="w-3.5 h-3.5" />
            {appSource(preflightApp) === "portable" ? "Remove folder" : "Continue"}
          </button>
        </footer>
      </div>
    </div>
  {/if}

  <!-- Remnants Modal Overlay -->
  {#if activeApp}
    <div
      class="fixed inset-0 z-50 bg-overlay-bg backdrop-blur-md flex items-center justify-center p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="remnants-title"
    >
      <div class="bg-surface-bg border border-border-default w-full max-w-2xl rounded-xl flex flex-col max-h-[85vh] shadow-2xl relative overflow-hidden animate-scale-up duration-200">
        <!-- Header -->
        <header class="p-6 border-b border-border-default flex items-center justify-between">
          <div>
            <h3 id="remnants-title" class="text-base font-bold text-text-primary">
              Deep Clean remnants for: <span class="text-accent">{activeApp.display_name}</span>
            </h3>
            <p class="text-xs text-text-muted mt-1">Review files, directories, and registry keys to be permanently purged.</p>
          </div>
          <button
            onclick={closeRemnantsModal}
            disabled={isPurging || uninstallStep === "uninstalling" || uninstallStep === "scanning"}
            class="p-1.5 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            aria-label="Close remnants dialog"
          >
            <X class="w-4 h-4" />
          </button>
        </header>

        <!-- Body -->
        <div class="flex-1 overflow-y-auto p-6 space-y-4">
          {#if uninstallStep === "uninstalling"}
            <div class="flex flex-col items-center justify-center py-12 gap-4 text-center animate-fade-in">
              <div class="w-12 h-12 rounded-full border-2 border-accent/20 border-t-accent animate-spin flex items-center justify-center">
                <Trash2 class="w-5 h-5 text-accent animate-pulse" />
              </div>
              <div>
                <h4 class="text-sm font-bold text-text-primary">Official Uninstaller Running</h4>
                <p class="text-xs text-text-muted mt-2 max-w-sm leading-relaxed font-sans">{uninstallStatusText}</p>
              </div>
            </div>
          {:else if uninstallStep === "scanning"}
            <div class="flex flex-col items-center justify-center py-12 gap-4 text-center animate-fade-in">
              <RefreshCw class="w-8 h-8 text-accent animate-spin" />
              <div>
                <h4 class="text-sm font-bold text-text-primary">Scanning Leftover Traces</h4>
                <p class="text-xs text-text-muted mt-2 max-w-sm leading-relaxed font-sans">{uninstallStatusText}</p>
              </div>
            </div>
          {:else}
            {#if purgeResult}
              <div class="p-6 rounded-lg bg-success/10 border border-success/20 flex flex-col items-center justify-center text-center gap-3">
                <div class="w-10 h-10 rounded-full bg-success/20 border border-success/30 flex items-center justify-center text-success animate-fade-in">
                  <Trash2 class="w-5 h-5" />
                </div>
                <div>
                  <h4 class="text-sm font-bold text-text-primary">Purge Completed!</h4>
                  <p class="text-xs text-text-muted mt-1">
                    Successfully removed <b class="text-success">{purgeResult.success}</b> remnants.
                    {#if purgeResult.fail > 0}
                      <span class="text-danger">Failed to remove {purgeResult.fail} items (locked or permission issue).</span>
                    {/if}
                  </p>
                </div>
              </div>
            {/if}

            {#if remnants.length === 0}
              {#if !purgeResult}
                <div class="p-8 text-center border border-dashed border-border-default rounded-lg bg-surface-bg/30 flex flex-col items-center justify-center">
                  <span class="text-sm font-sans text-text-primary font-medium">No Remnants Detected!</span>
                  <span class="text-xs text-text-muted mt-1">This application was successfully uninstalled without leaving residual traces.</span>
                </div>
              {/if}
            {:else}
              <!-- Toolbar selection -->
              <div class="flex items-center justify-between pb-2 text-xs border-b border-border-default">
                <div class="flex gap-2">
                  <button
                    onclick={() => selectAll(true)}
                    class="text-accent hover:text-accent-hover font-semibold"
                  >
                    Select All
                  </button>
                  <span class="text-border-default">|</span>
                  <button
                    onclick={() => selectAll(false)}
                    class="text-text-secondary hover:text-text-primary font-semibold"
                  >
                    Deselect All
                  </button>
                </div>
                <span class="text-text-muted font-mono">
                  {remnants.filter(r => selectedRemnants[r.path]).length} of {remnants.length} items selected
                </span>
              </div>

              <!-- Grouped list -->
              <div class="space-y-4">
                {#each remnantGroups as group}
                  <section class="space-y-2">
                    <div class="flex items-center justify-between gap-3">
                      <h4 class="text-[11px] font-mono uppercase tracking-wider text-text-muted">
                        {group.label} <span class="text-text-secondary">({group.items.length})</span>
                      </h4>
                      <div class="flex items-center gap-2 text-[10px]">
                        <button onclick={() => selectGroup(group.type, true)} class="text-accent hover:text-accent-hover font-semibold">All</button>
                        <span class="text-border-default">|</span>
                        <button onclick={() => selectGroup(group.type, false)} class="text-text-muted hover:text-text-primary font-semibold">None</button>
                      </div>
                    </div>

                    {#each group.items as item}
                      <button
                        onclick={() => toggleSelection(item.path)}
                        class="w-full flex items-start gap-3 p-3 rounded-lg border text-left transition-all hover:bg-elevated-bg/50
                          {selectedRemnants[item.path] ? 'bg-accent-subtle/30 border-accent/20' : 'bg-surface-bg border-border-default'}"
                      >
                        <div class="mt-0.5 text-accent flex-shrink-0">
                          {#if selectedRemnants[item.path]}
                            <CheckSquare class="w-4 h-4 text-accent" />
                          {:else}
                            <Square class="w-4 h-4 text-text-muted" />
                          {/if}
                        </div>

                        <div class="w-8 h-8 rounded bg-app-bg border border-border-default flex items-center justify-center text-text-secondary flex-shrink-0">
                          {#if item.item_type === 'Directory'}
                            <Folder class="w-4.5 h-4.5 text-warning" />
                          {:else if item.item_type === 'RegistryKey' || item.item_type === 'RegistryValue'}
                            <Database class="w-4.5 h-4.5 text-brand-purple" />
                          {:else if item.item_type === 'ScheduledTask'}
                            <RefreshCw class="w-4.5 h-4.5 text-info" />
                          {:else}
                            <File class="w-4.5 h-4.5 text-info" />
                          {/if}
                        </div>

                        <div class="flex-1 min-w-0">
                          <span class="block text-xs font-mono font-medium text-text-primary break-all">{item.path}</span>
                          <span class="block text-[10px] text-text-muted mt-1">{remnantReason(item)}</span>
                          <div class="flex flex-wrap items-center gap-2 mt-1">
                            <span class="text-[9px] font-mono uppercase px-1 rounded bg-app-bg border border-border-default text-text-muted">
                              {item.item_type}
                            </span>
                            {#if item.size > 0}
                              <span class="text-[9px] font-mono text-text-muted">{formatSize(item.size)}</span>
                            {/if}
                            <span class="text-[9px] font-mono px-1 rounded border font-semibold
                              {item.confidence === 'VeryHigh' ? 'bg-emerald-950/30 text-emerald-400 border-emerald-800/40' : ''}
                              {item.confidence === 'High' ? 'bg-blue-950/30 text-blue-400 border-blue-800/40' : ''}
                              {item.confidence === 'Medium' ? 'bg-amber-950/30 text-amber-400 border-amber-800/40' : ''}
                              {item.confidence === 'Low' ? 'bg-rose-950/30 text-rose-400 border-rose-800/40' : ''}"
                            >
                              {item.confidence} ({item.score}%)
                            </span>
                          </div>
                        </div>
                      </button>
                    {/each}
                  </section>
                {/each}
              </div>
            {/if}
          {/if}
        </div>

        <!-- Footer -->
        {#if !purgeResult && remnants.length > 0}
          <footer class="p-6 border-t border-border-default flex justify-between items-center bg-sidebar-bg">
            <span class="text-xs font-mono text-text-secondary">
              Total selected: <b class="text-accent">{formatSize(remnants.filter(r => selectedRemnants[r.path]).reduce((acc, curr) => acc + curr.size, 0))}</b>
            </span>
            <div class="flex gap-3">
              <button
                onclick={closeRemnantsModal}
                class="px-4 py-2 rounded-lg text-xs font-semibold bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-all"
              >
                Cancel
              </button>
              <button
                onclick={performPurge}
                disabled={isPurging || remnants.filter(r => selectedRemnants[r.path]).length === 0}
                class="flex items-center gap-2 px-5 py-2 rounded-lg text-xs font-semibold bg-accent hover:bg-accent-hover text-white disabled:opacity-50 active:scale-95 transition-all shadow"
              >
                {#if isPurging}
                  <RefreshCw class="w-3.5 h-3.5 animate-spin" />
                  Purging remnants...
                {:else}
                  <Trash2 class="w-3.5 h-3.5" />
                  Purge Selected Remnants
                {/if}
              </button>
            </div>
          </footer>
        {/if}
      </div>
    </div>
  {/if}

  <!-- Floating Bulk Action Bar -->
  {#if selectedVisibleAppsCount >= 2 && !isBulkUninstalling}
    <div class="fixed bottom-6 left-1/2 -translate-x-1/2 z-40 w-full max-w-2xl px-4 animate-slide-up">
      <div class="backdrop-blur-md bg-sidebar-bg/95 border border-border-strong rounded-xl p-4 shadow-2xl flex items-center justify-between gap-4">
        <div class="flex items-center gap-3">
          <div class="w-8 h-8 rounded-lg bg-accent/10 border border-accent/20 flex items-center justify-center text-accent">
            <CheckSquare class="w-4.5 h-4.5" />
          </div>
          <div>
            <h4 class="text-xs font-bold text-text-primary">{selectedVisibleAppsCount} apps selected</h4>
            <p class="text-[10px] text-text-muted mt-0.5 font-sans">
              Ready for batch silent uninstallation
              {#if selectedAppsCount > selectedVisibleAppsCount}
                · {selectedAppsCount - selectedVisibleAppsCount} hidden ignored
              {/if}
            </p>
          </div>
        </div>

        <div class="flex items-center gap-4">
          <!-- Auto-Purge Toggle -->
          <label class="flex items-center gap-2 cursor-pointer select-none">
            <input
              type="checkbox"
              bind:checked={bulkAutoPurge}
              class="rounded border-border-default text-accent focus:ring-accent bg-surface-bg cursor-pointer w-4 h-4"
            />
            <div class="flex flex-col">
              <span class="text-xs font-semibold text-text-secondary">Auto-Purge Leftovers</span>
              <span class="text-[9px] text-text-muted font-sans font-medium">Delete remnants silently</span>
            </div>
          </label>

          <div class="h-8 w-[1px] bg-border-default"></div>

          <div class="flex items-center gap-2">
            <button
              onclick={clearAppSelection}
              class="px-3 py-1.5 rounded-lg text-xs font-semibold bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-all active:scale-95 cursor-pointer"
            >
              Cancel
            </button>
            <button
              onclick={startBulkUninstall}
              class="flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-xs font-bold bg-accent hover:bg-accent-hover text-white transition-all active:scale-95 shadow cursor-pointer"
            >
              <Trash2 class="w-3.5 h-3.5" />
              Bulk Silent Uninstall
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- Bulk Uninstall Modal -->
  {#if isBulkUninstalling}
    <div
      class="absolute inset-0 bg-app-bg/60 backdrop-blur-sm z-50 flex items-center justify-center p-6 animate-fade-in"
      role="dialog"
      aria-modal="true"
      aria-labelledby="bulk-title"
    >
      <div class="bg-sidebar-bg border border-border-strong w-full max-w-3xl rounded-xl shadow-2xl overflow-hidden flex flex-col max-h-[85vh] animate-scale-up">
        <!-- Header -->
        <header class="p-6 border-b border-border-default flex items-center justify-between">
          <div>
            <h3 id="bulk-title" class="text-base font-bold text-text-primary">
              Bulk Silent Uninstallation
            </h3>
            <p class="text-xs text-text-muted mt-1">
              {#if bulkUninstallStep === "running"}
                Processing <b>{bulkUninstallCurrent}/{bulkUninstallTotal}</b>: <span class="text-accent">{bulkUninstallCurrentAppName}</span>
              {:else if bulkUninstallStep === "remnants_list"}
                Review remaining leftovers from batch uninstallation
              {:else}
                Batch operations completed
              {/if}
            </p>
          </div>
          {#if bulkUninstallStep === "done" || bulkUninstallStep === "error"}
            <button
              onclick={closeBulkModal}
              class="p-1.5 rounded-lg bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-colors cursor-pointer"
              aria-label="Close bulk uninstallation dialog"
            >
              <X class="w-4 h-4" />
            </button>
          {/if}
        </header>

        <!-- Progress Bar -->
        {#if bulkUninstallStep === "running" && bulkUninstallTotal > 0}
          <div class="h-1 bg-border-default w-full relative overflow-hidden">
            <div
              class="h-full bg-accent transition-all duration-300"
              style="width: {(bulkUninstallCurrent / bulkUninstallTotal) * 100}%"
            ></div>
          </div>
        {/if}

        <!-- Body -->
        <div class="flex-1 overflow-y-auto p-6 space-y-4">
          {#if bulkUninstallStep === "running"}
            <div class="flex flex-col gap-3">
              <div class="flex items-center gap-3 p-3.5 rounded-lg bg-accent/5 border border-accent/10">
                <RefreshCw class="w-5 h-5 text-accent animate-spin flex-shrink-0" />
                <div class="flex-1 min-w-0">
                  <div class="flex items-center justify-between text-xs font-semibold text-text-primary">
                    <span>{bulkUninstallCurrentAppName}</span>
                    <span class="font-mono text-accent">{bulkUninstallCurrent}/{bulkUninstallTotal}</span>
                  </div>
                  <p class="text-[11px] text-text-muted mt-1 truncate">{bulkUninstallStatusMessage}</p>
                </div>
              </div>

              <!-- Console-style Log -->
              <h4 class="text-xs font-bold text-text-secondary uppercase tracking-wider font-mono">Uninstall Log</h4>
              <div class="bg-app-bg border border-border-default rounded-lg p-4 font-mono text-[11px] text-text-muted overflow-y-auto h-[250px] space-y-1.5 select-text">
                {#each bulkUninstallLog as line}
                  <div class="leading-relaxed border-b border-border-default/20 pb-1 last:border-b-0 break-all">{line}</div>
                {/each}
              </div>
            </div>

          {:else if bulkUninstallStep === "remnants_list"}
            <!-- Residual Clean list -->
            <div class="space-y-3">
              <div class="p-4 rounded-lg bg-amber-950/20 border border-amber-900/40 text-amber-300 text-xs flex gap-3 leading-relaxed">
                <span class="text-base">⚠️</span>
                <div>
                  <h4 class="font-bold">Remaining Leftovers Detected</h4>
                  <p class="mt-1 text-text-muted">The uninstaller completed, but some confidence-level remnants couldn't be auto-purged (or auto-purge was disabled). Review and select below to clean them permanently.</p>
                </div>
              </div>

              <!-- Selection Toolbar -->
              <div class="flex items-center justify-between pb-1 text-xs border-b border-border-default">
                <div class="flex gap-2">
                  <button
                    onclick={() => {
                      bulkRemnants.forEach(r => selectedBulkRemnants[r.path] = true);
                    }}
                    class="text-accent hover:text-accent-hover font-semibold cursor-pointer"
                  >
                    Select All
                  </button>
                  <span class="text-border-default">|</span>
                  <button
                    onclick={() => {
                      selectedBulkRemnants = {};
                    }}
                    class="text-text-secondary hover:text-text-primary font-semibold cursor-pointer"
                  >
                    Deselect All
                  </button>
                </div>
                <span class="text-text-muted font-mono">
                  {Object.values(selectedBulkRemnants).filter(Boolean).length} of {bulkRemnants.length} items selected
                </span>
              </div>

              <!-- Remnants List -->
              <div class="space-y-2 max-h-[280px] overflow-y-auto">
                {#each bulkRemnants as item}
                  <button
                    onclick={() => selectedBulkRemnants[item.path] = !selectedBulkRemnants[item.path]}
                    class="w-full flex items-start gap-3 p-3 rounded-lg border text-left transition-all hover:bg-elevated-bg/50 cursor-pointer
                      {selectedBulkRemnants[item.path] ? 'bg-accent-subtle/30 border-accent/20' : 'bg-surface-bg border-border-default'}"
                  >
                    <div class="mt-0.5 text-accent flex-shrink-0">
                      {#if selectedBulkRemnants[item.path]}
                        <CheckSquare class="w-4 h-4 text-accent" />
                      {:else}
                        <Square class="w-4 h-4 text-text-muted" />
                      {/if}
                    </div>

                    <div class="w-8 h-8 rounded bg-app-bg border border-border-default flex items-center justify-center text-text-secondary flex-shrink-0">
                      {#if item.item_type === 'Directory'}
                        <Folder class="w-4.5 h-4.5 text-warning" />
                      {:else if item.item_type === 'RegistryKey'}
                        <Database class="w-4.5 h-4.5 text-brand-purple" />
                      {:else}
                        <File class="w-4.5 h-4.5 text-info" />
                      {/if}
                    </div>

                    <div class="flex-1 min-w-0">
                      <span class="block text-xs font-mono font-medium text-text-primary break-all">{item.path}</span>
                      <div class="flex flex-wrap items-center gap-2 mt-1">
                        <span class="text-[9px] font-mono uppercase px-1 rounded bg-app-bg border border-border-default text-text-muted">
                          {item.item_type}
                        </span>
                        {#if item.size > 0}
                          <span class="text-[9px] font-mono text-text-muted">{formatSize(item.size)}</span>
                        {/if}
                        <span class="text-[9px] font-mono px-1 rounded border font-semibold
                          {item.confidence === 'VeryHigh' ? 'bg-emerald-950/30 text-emerald-400 border-emerald-800/40' : ''}
                          {item.confidence === 'High' ? 'bg-blue-950/30 text-blue-400 border-blue-800/40' : ''}
                          {item.confidence === 'Medium' ? 'bg-amber-950/30 text-amber-400 border-amber-800/40' : ''}
                          {item.confidence === 'Low' ? 'bg-rose-950/30 text-rose-400 border-rose-800/40' : ''}"
                        >
                          {item.confidence} ({item.score}%)
                        </span>
                      </div>
                    </div>
                  </button>
                {/each}
              </div>
            </div>

          {:else if bulkUninstallStep === "error"}
            <div class="py-12 flex flex-col items-center justify-center text-center gap-4 animate-fade-in">
              <div class="w-16 h-16 rounded-full bg-danger/10 border border-danger/20 flex items-center justify-center text-danger shadow-inner">
                <X class="w-8 h-8" />
              </div>
              <div>
                <h4 class="text-base font-bold text-text-primary">Bulk Uninstall Failed</h4>
                <p class="text-xs text-text-muted mt-2 max-w-sm leading-relaxed break-all">
                  {bulkUninstallError || "The operation could not be completed."}
                </p>
                <p class="text-[11px] text-text-muted mt-2">
                  Some applications may have been processed before the error. The list was refreshed.
                </p>
              </div>
            </div>

          {:else if bulkUninstallStep === "done"}
            <div class="py-12 flex flex-col items-center justify-center text-center gap-4 animate-fade-in">
              <div class="w-16 h-16 rounded-full bg-success/10 border border-success/20 flex items-center justify-center text-success shadow-inner">
                <svg class="w-8 h-8" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="20 6 9 17 4 12"/>
                </svg>
              </div>
              <div>
                <h4 class="text-base font-bold text-text-primary">All Applications Processed!</h4>
                <p class="text-xs text-text-muted mt-2 max-w-sm leading-relaxed">
                  Bulk uninstallation and leftovers cleaning completed successfully.
                </p>
              </div>
            </div>
          {/if}
        </div>

        <!-- Footer -->
        {#if bulkUninstallStep === "remnants_list" || bulkUninstallStep === "done" || bulkUninstallStep === "error"}
          <footer class="p-6 border-t border-border-default flex justify-end items-center gap-3 bg-sidebar-bg">
            {#if bulkUninstallStep === "remnants_list"}
              <button
                onclick={() => bulkUninstallStep = "done"}
                class="px-4 py-2 rounded-lg text-xs font-semibold bg-surface-bg border border-border-default hover:bg-elevated-bg text-text-secondary hover:text-text-primary transition-all cursor-pointer"
              >
                Skip Cleaning
              </button>
              <button
                onclick={performBulkRemnantsPurge}
                disabled={isPurging}
                class="flex items-center gap-1.5 px-5 py-2 rounded-lg text-xs font-semibold bg-accent hover:bg-accent-hover text-white transition-all active:scale-95 disabled:opacity-50 cursor-pointer"
              >
                {#if isPurging}
                  <RefreshCw class="w-3.5 h-3.5 animate-spin" />
                  Purging leftovers...
                {:else}
                  <Trash2 class="w-3.5 h-3.5" />
                  Clean Leftover Traces
                {/if}
              </button>
            {:else}
              <button
                onclick={closeBulkModal}
                class="px-6 py-2 rounded-lg text-xs font-semibold bg-accent hover:bg-accent-hover text-white transition-all active:scale-95 cursor-pointer"
              >
                Done
              </button>
            {/if}
          </footer>
        {/if}
      </div>
    </div>
  {/if}
</div>
