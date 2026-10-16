<script lang="ts">
	// The player bar on a phone (Android). Collapsed it is a strip: cover, title, play/pause and
	// next, with the progress as a hairline along its top; tapping the strip opens the player view.
	// With the view open it grows the full transport (seek, shuffle, repeat, like) and the queue and
	// lyrics buttons, which open the same floating panels the desktop's untabbed layout uses.
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		PreviousIcon,
		NextIcon,
		PlayIcon,
		PauseIcon,
		ShuffleIcon,
		RepeatIcon,
		RepeatOne01Icon,
		Queue01Icon,
		Mic01Icon,
		FavouriteIcon,
		MusicNote01Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import { np, playback, cycleRepeat, toggleNowPlayingRating } from '$lib/player.svelte';
	import { thumb } from '$lib/thumb';
	import { t } from '$lib/i18n.svelte';

	let {
		onToggleQueue,
		queueOpen,
		onToggleLyrics,
		lyricsOpen
	}: {
		onToggleQueue: () => void;
		queueOpen: boolean;
		onToggleLyrics: () => void;
		lyricsOpen: boolean;
	} = $props();

	const shuffleOn = $derived(playback.queue.shuffle ?? false);
	const repeat = $derived(playback.queue.repeat ?? 'off');

	// Same as the desktop bar: hold the dragged value so position ticks can't yank the thumb, and
	// seek once, on release.
	let seekDrag = $state<number | null>(null);
	const shown = $derived(seekDrag ?? playback.position);
	const pct = $derived(playback.duration ? Math.min(100, (shown / playback.duration) * 100) : 0);

	const fmt = (secs: number) => {
		if (!secs || secs < 0) return '0:00';
		const s = Math.floor(secs);
		const h = Math.floor(s / 3600);
		const m = Math.floor((s % 3600) / 60);
		const ss = String(s % 60).padStart(2, '0');
		return h ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
	};
</script>

{#snippet playPause(size: string)}
	<button
		class="flex {size} shrink-0 items-center justify-center rounded-full bg-primary text-primary-foreground"
		onclick={() => api.togglePause()}
		aria-label={playback.paused ? t('player.play') : t('player.pause')}
	>
		<!-- altIcon/showAlt: `icon` is frozen at mount. -->
		<HugeiconsIcon icon={PauseIcon} altIcon={PlayIcon} showAlt={playback.paused} class="h-5 w-5" />
	</button>
{/snippet}

<footer class="shrink-0 border-t bg-card/95">
	{#if np.open}
		<div class="flex flex-col gap-2 px-4 pb-3 pt-2">
			<div class="flex items-center gap-2 text-xs text-muted-foreground">
				<span class="min-w-[5ch] text-right tabular-nums">{fmt(shown)}</span>
				<input
					type="range"
					class="range flex-1"
					style="--pct:{pct}%"
					min="0"
					max={playback.duration || 0}
					value={shown}
					oninput={(e) => (seekDrag = Number(e.currentTarget.value))}
					onchange={(e) => {
						const v = Number(e.currentTarget.value);
						playback.position = v;
						seekDrag = null;
						api.seek(v);
					}}
					aria-label={t('player.seek')}
				/>
				<span class="min-w-[5ch] tabular-nums">{fmt(playback.duration)}</span>
			</div>
			<div class="flex items-center justify-between">
				<button
					class="flex size-10 items-center justify-center"
					onclick={() => api.toggleShuffle()}
					aria-label={t('player.shuffle')}
					aria-pressed={shuffleOn}
				>
					<HugeiconsIcon
						icon={ShuffleIcon}
						class="h-5 w-5 {shuffleOn ? 'text-primary' : 'text-muted-foreground'}"
					/>
				</button>
				<button
					class="flex size-11 items-center justify-center"
					onclick={() => api.prevTrack()}
					aria-label={t('player.previous')}
				>
					<HugeiconsIcon icon={PreviousIcon} class="h-6 w-6" />
				</button>
				{@render playPause('size-14')}
				<button
					class="flex size-11 items-center justify-center"
					onclick={() => api.nextTrack()}
					aria-label={t('player.next')}
				>
					<HugeiconsIcon icon={NextIcon} class="h-6 w-6" />
				</button>
				<button
					class="flex size-10 items-center justify-center"
					onclick={cycleRepeat}
					aria-pressed={repeat !== 'off'}
					aria-label={t('player.repeat_state', {
						state:
							repeat === 'off'
								? t('player.repeat_off')
								: repeat === 'one'
									? t('player.repeat_one')
									: t('player.repeat_all')
					})}
				>
					<HugeiconsIcon
						icon={RepeatIcon}
						altIcon={RepeatOne01Icon}
						showAlt={repeat === 'one'}
						class="h-5 w-5 {repeat !== 'off' ? 'text-primary' : 'text-muted-foreground'}"
					/>
				</button>
			</div>
			<div class="flex items-center justify-around">
				{#if playback.now && !api.isLocalId(playback.now.videoId)}
					<button
						class="flex size-10 items-center justify-center"
						onclick={() => toggleNowPlayingRating()}
						aria-label={t('common.like')}
					>
						<HugeiconsIcon
							icon={FavouriteIcon}
							class="h-5 w-5 {playback.rating === 'like'
								? 'fill-current text-primary'
								: 'text-muted-foreground'}"
						/>
					</button>
				{/if}
				<button
					class="flex size-10 items-center justify-center"
					onclick={onToggleLyrics}
					aria-label={t('player.lyrics')}
					aria-pressed={lyricsOpen}
				>
					<HugeiconsIcon
						icon={Mic01Icon}
						class="h-5 w-5 {lyricsOpen ? 'text-primary' : 'text-muted-foreground'}"
					/>
				</button>
				<button
					class="flex size-10 items-center justify-center"
					onclick={onToggleQueue}
					aria-label={t('player.queue')}
					aria-pressed={queueOpen}
				>
					<HugeiconsIcon
						icon={Queue01Icon}
						class="h-5 w-5 {queueOpen ? 'text-primary' : 'text-muted-foreground'}"
					/>
				</button>
			</div>
		</div>
	{:else}
		<div class="relative">
			<!-- Progress, read-only here: seeking wants the bigger target in the open view. -->
			<div class="absolute inset-x-0 top-0 h-0.5 bg-muted">
				<div class="h-full bg-primary" style="width:{pct}%"></div>
			</div>
			<div class="flex items-center gap-3 px-3 py-2">
				<button
					class="flex min-w-0 flex-1 items-center gap-3 text-left"
					onclick={() => (np.open = true)}
					aria-label={t('player.open_player')}
				>
					{#if playback.now?.thumbnail}
						<img
							src={thumb(playback.now.thumbnail, 120)}
							alt=""
							class="h-11 w-11 shrink-0 rounded-md object-cover"
						/>
					{:else}
						<div
							class="flex h-11 w-11 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground"
						>
							<HugeiconsIcon icon={MusicNote01Icon} class="h-5 w-5" />
						</div>
					{/if}
					<span class="min-w-0 flex-1">
						<span class="block truncate text-sm font-medium">{playback.now?.title ?? ''}</span>
						<span class="block truncate text-xs text-muted-foreground">
							{playback.now?.artists ?? ''}
						</span>
					</span>
				</button>
				{@render playPause('size-10')}
				<button
					class="flex size-10 shrink-0 items-center justify-center"
					onclick={() => api.nextTrack()}
					aria-label={t('player.next')}
				>
					<HugeiconsIcon icon={NextIcon} class="h-5 w-5" />
				</button>
			</div>
		</div>
	{/if}
</footer>
