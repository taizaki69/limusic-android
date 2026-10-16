// Which shell the SPA runs in. The Android build reuses this whole UI; what changes there is the
// chrome around it (no window frame, a bottom nav instead of the sidebar, a compact player bar)
// and the desktop-only features it hides (mini player, global hotkeys, Discord, self-update).
//
// Read from the user agent, so it is known synchronously before the first paint: Android's
// WebView always says "Android", and no desktop webview Limusic runs in ever does.
import { browser } from '$app/environment';

export const mobile = browser && /Android/i.test(navigator.userAgent);
