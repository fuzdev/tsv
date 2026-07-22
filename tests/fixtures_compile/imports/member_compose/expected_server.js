import * as $ from 'svelte/internal/server';
import { x } from './x.js';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		$$renderer.push(`<p>${$.escape(x.y)}</p>`);
	});
}
