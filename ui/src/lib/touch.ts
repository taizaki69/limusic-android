// Touch-only behaviour for the Android build (`mobile`, platform.ts).
//
// A long-press is the phone's right-click: it opens the item menu every `[data-ctx]` host already
// wires to `contextmenu` (`ctxHost` in menu.ts). Android's WebView is meant to fire that event on
// its own, but it doesn't on a draggable element (a card), it can be pre-empted by a text selection,
// and whether it comes at all depends on the WebView build. So the press is timed here, a
// `contextmenu` is dispatched at the finger, and a native one arriving for the same press is
// swallowed so the menu never opens twice.

const HOLD_MS = 450;
/** A finger that wanders further than this (px) is scrolling, not holding. */
const SLOP = 10;

export function initTouch() {
	let timer = 0;
	let start: { x: number; y: number } | null = null;
	/** The current press already opened a menu: its native contextmenu and its click are spent. */
	let fired = false;

	const cancel = () => {
		clearTimeout(timer);
		start = null;
	};

	const onStart = (e: TouchEvent) => {
		cancel();
		fired = false;
		if (e.touches.length !== 1) return;
		const t = e.touches[0];
		const target = e.target;
		if (!(target instanceof Element) || !target.closest('[data-ctx]')) return;
		// A text field keeps the platform's own hold (cursor handles, paste).
		if (target.closest('input, textarea, [contenteditable]')) return;
		start = { x: t.clientX, y: t.clientY };
		timer = window.setTimeout(() => {
			start = null;
			fired = true;
			navigator.vibrate?.(12);
			target.dispatchEvent(
				new MouseEvent('contextmenu', {
					bubbles: true,
					cancelable: true,
					clientX: t.clientX,
					clientY: t.clientY,
					button: 2
				})
			);
		}, HOLD_MS);
	};

	const onMove = (e: TouchEvent) => {
		if (!start) return;
		const t = e.touches[0];
		if (Math.hypot(t.clientX - start.x, t.clientY - start.y) > SLOP) cancel();
	};

	// The platform's own contextmenu for a press that already opened ours. Trusted events only: ours
	// is synthetic and has to get through.
	const onNativeMenu = (e: MouseEvent) => {
		if (!e.isTrusted) return;
		if (fired) {
			e.preventDefault();
			e.stopImmediatePropagation();
			return;
		}
		// The platform beat the timer to it: let its event open the menu, and don't fire a second.
		cancel();
		fired = true;
	};

	// Lifting the finger after a hold would otherwise also "tap" what was held (play the song,
	// open the album) underneath the menu that just opened.
	const onClick = (e: MouseEvent) => {
		if (!fired) return;
		fired = false;
		e.preventDefault();
		e.stopImmediatePropagation();
	};

	const opts = { capture: true, passive: true };
	addEventListener('touchstart', onStart, opts);
	addEventListener('touchmove', onMove, opts);
	addEventListener('touchend', cancel, opts);
	addEventListener('touchcancel', cancel, opts);
	addEventListener('scroll', cancel, opts);
	addEventListener('contextmenu', onNativeMenu, { capture: true });
	addEventListener('click', onClick, { capture: true });
}
