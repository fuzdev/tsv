import * as $ from 'svelte/internal/server';
import { api } from './api.js';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		$$renderer.push(`<p>${$.escape(api.foo())}</p>`);
	});
}
