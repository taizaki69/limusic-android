<script lang="ts">
	// The phone's stand-in for the sidebar (Android): the same three destinations plus history and
	// settings, as a bottom bar under the player. The library's playlists, which the sidebar lists,
	// are one tap away on /library.
	import { page } from '$app/state';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Home01Icon,
		Search01Icon,
		LibraryIcon,
		HistoryIcon,
		Settings01Icon
	} from '@hugeicons/core-free-icons';
	import { np, ui } from '$lib/player.svelte';
	import { t } from '$lib/i18n.svelte';

	const nav = $derived([
		{ href: '/', label: t('nav.home'), icon: Home01Icon },
		{ href: '/search', label: t('nav.search'), icon: Search01Icon },
		{ href: '/library', label: t('nav.library'), icon: LibraryIcon },
		{ href: '/history', label: t('nav.history'), icon: HistoryIcon }
	]);
	const isActive = (href: string) =>
		href === '/' ? page.url.pathname === '/' : page.url.pathname.startsWith(href);
</script>

<nav class="flex h-14 shrink-0 items-stretch border-t bg-sidebar text-sidebar-foreground">
	{#each nav as n (n.href)}
		<!-- Leaving for another page closes the player view, as a tab bar does. -->
		<a
			href={n.href}
			onclick={() => (np.open = false)}
			class="flex flex-1 flex-col items-center justify-center gap-0.5 text-[11px] transition-colors {isActive(
				n.href
			) && !np.open
				? 'text-primary'
				: 'text-muted-foreground'}"
			aria-current={isActive(n.href) ? 'page' : undefined}
		>
			<HugeiconsIcon icon={n.icon} class="h-5 w-5" />
			<span class="max-w-full truncate px-1">{n.label}</span>
		</a>
	{/each}
	<button
		class="flex flex-1 flex-col items-center justify-center gap-0.5 text-[11px] text-muted-foreground"
		onclick={() => (ui.settingsOpen = true)}
	>
		<HugeiconsIcon icon={Settings01Icon} class="h-5 w-5" />
		<span class="max-w-full truncate px-1">{t('nav.settings')}</span>
	</button>
</nav>
