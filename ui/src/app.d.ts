// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		interface PageState {
			/** Android: a sheet (player, settings) is open over the page; back closes it. */
			overlay?: boolean;
		}
		// interface Platform {}
	}
}

export {};
