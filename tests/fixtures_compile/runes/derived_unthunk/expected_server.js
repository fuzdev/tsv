import * as $ from 'svelte/internal/server';
import { get_library } from './library.ts';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		const library = $.derived(get_library);
		const scoped = $.derived(() => get_library().name);
		$$renderer.push(`<p>${$.escape(library())}${$.escape(scoped())}</p>`);
	});
}
