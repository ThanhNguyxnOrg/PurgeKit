<!-- src/lib/components/Toast.svelte -->
<script lang="ts">
  import { toast } from "../toast.svelte";
  import { CheckCircle, AlertTriangle, Info, AlertOctagon, X } from "@lucide/svelte";
</script>

<div class="fixed bottom-6 right-6 z-[100] flex flex-col gap-3 max-w-sm w-full pointer-events-none" role="status" aria-live="polite" aria-atomic="false">
  {#each toast.toasts as t (t.id)}
    <div
      class="flex items-start gap-3 p-4 rounded-xl border backdrop-blur-md shadow-lg transition-all duration-300 pointer-events-auto
        {t.type === 'success' ? 'bg-surface-bg/95 border-success/40 text-success' : ''}
        {t.type === 'error' ? 'bg-surface-bg/95 border-danger/40 text-danger' : ''}
        {t.type === 'warning' ? 'bg-surface-bg/95 border-warning/40 text-warning' : ''}
        {t.type === 'info' ? 'bg-surface-bg/95 border-info/40 text-info' : ''}
       animate-fade-in-up"
    >
      <div class="mt-0.5 flex-shrink-0">
        {#if t.type === 'success'}
          <CheckCircle class="w-5 h-5 text-success" />
        {:else if t.type === 'error'}
          <AlertOctagon class="w-5 h-5 text-danger" />
        {:else if t.type === 'warning'}
          <AlertTriangle class="w-5 h-5 text-warning" />
        {:else}
          <Info class="w-5 h-5 text-info" />
        {/if}
      </div>
      <div class="flex-1 min-w-0">
        <p class="text-xs font-medium font-sans leading-relaxed break-words">{t.message}</p>
      </div>
      <button
        onclick={() => toast.remove(t.id)}
        class="text-text-secondary hover:text-text-primary p-0.5 rounded transition-colors cursor-pointer"
        aria-label="Dismiss notification"
      >
        <X class="w-3.5 h-3.5" />
      </button>
    </div>
  {/each}
</div>

<style>
  @keyframes fadeInUp {
    from {
      opacity: 0;
      transform: translateY(1rem);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  .animate-fade-in-up {
    animation: fadeInUp 0.2s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }
</style>
