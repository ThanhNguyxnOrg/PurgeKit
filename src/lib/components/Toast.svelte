<!-- src/lib/components/Toast.svelte -->
<script lang="ts">
  import { toast } from "../toast";
  import { CheckCircle, AlertTriangle, Info, AlertOctagon, X } from "@lucide/svelte";
</script>

<div class="fixed bottom-6 right-6 z-[100] flex flex-col gap-3 max-w-sm w-full pointer-events-none">
  {#each toast.toasts as t (t.id)}
    <div
      class="flex items-start gap-3 p-4 rounded-xl border backdrop-blur-md shadow-lg transition-all duration-300 pointer-events-auto
        {t.type === 'success' ? 'bg-emerald-950/80 border-emerald-800/40 text-emerald-300' : ''}
        {t.type === 'error' ? 'bg-rose-950/80 border-rose-800/40 text-rose-300' : ''}
        {t.type === 'warning' ? 'bg-amber-950/80 border-amber-800/40 text-amber-300' : ''}
        {t.type === 'info' ? 'bg-blue-950/80 border-blue-800/40 text-blue-300' : ''}
       animate-fade-in-up"
    >
      <div class="mt-0.5 flex-shrink-0">
        {#if t.type === 'success'}
          <CheckCircle class="w-5 h-5 text-emerald-400" />
        {:else if t.type === 'error'}
          <AlertOctagon class="w-5 h-5 text-rose-400" />
        {:else if t.type === 'warning'}
          <AlertTriangle class="w-5 h-5 text-amber-400" />
        {:else}
          <Info class="w-5 h-5 text-blue-400" />
        {/if}
      </div>
      <div class="flex-1 min-w-0">
        <p class="text-xs font-medium font-sans leading-relaxed break-words">{t.message}</p>
      </div>
      <button
        onclick={() => toast.remove(t.id)}
        class="text-text-secondary hover:text-text-primary p-0.5 rounded transition-colors cursor-pointer"
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
