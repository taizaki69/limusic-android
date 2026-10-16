<script lang="ts">
	import { fade, fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import QueueList from './QueueList.svelte';
	import { t } from '$lib/i18n.svelte';
	import { mobile } from '$lib/platform';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { ArrowDown01Icon } from '@hugeicons/core-free-icons';

	let { onClose }: { onClose: () => void } = $props();
</script>

<!-- The panel always floats over the content (see the `relative` wrapper in +layout) rather than
     squeezing it into a column: two docked panels left the page too narrow to read, and a page you
     can't use behind a panel you opened on purpose is the better trade. Below lg a scrim dismisses
     it; at lg+ the content stays visible underneath and the player bar's button closes it. -->
<!-- Not on a phone, where the sheet covers everything and has its own ⌄. -->
{#if !mobile}
<button
	class="absolute inset-0 z-20 cursor-default bg-black/40 lg:hidden"
	onclick={onClose}
	aria-label={t('a11y.close_queue')}
	transition:fade={{ duration: 150 }}
></button>
{/if}
<!-- A phone gets a full-screen sheet with its own way down: a side panel there left a sliver of
     the view behind it, and the scrim was the only (invisible) way out. -->
<aside
	transition:fly={mobile ? { y: 48, duration: 220, easing: cubicOut } : { x: 32, duration: 220, easing: cubicOut }}
	class="absolute z-30 flex flex-col shadow-2xl {mobile
		? 'inset-0 bg-background'
		: 'inset-y-0 right-0 h-full w-80 max-w-[80vw] border-l bg-card/95'}"
>
	{#if mobile}
		<div class="flex items-center gap-2 border-b px-2 py-1.5">
			<button
				onclick={onClose}
				class="flex size-10 items-center justify-center rounded-full text-muted-foreground"
				aria-label={t('a11y.close_queue')}
			>
				<HugeiconsIcon icon={ArrowDown01Icon} class="h-5 w-5" />
			</button>
			<h2 class="font-heading text-base font-semibold">{t('queue.title')}</h2>
		</div>
	{:else}
		<h2 class="border-b px-4 py-3 font-heading text-sm font-semibold">{t('queue.title')}</h2>
	{/if}
	<QueueList />
</aside>
