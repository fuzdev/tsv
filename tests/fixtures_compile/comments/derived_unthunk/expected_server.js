import * as $ from 'svelte/internal/server';
import { get_thing } from './thing.js';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let b = $.derived(get_thing);
		// after unthunk
		let c = 3;
		$$renderer.push(`<p>${$.escape(b())}3</p>`);
	});
}
