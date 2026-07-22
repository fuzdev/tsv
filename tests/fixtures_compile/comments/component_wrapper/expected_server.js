import * as $ from 'svelte/internal/server';
import Child from './Child.svelte';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		// forces the wrapper
		let d = new Date();
		Child($$renderer, { year: d.getFullYear() });
	});
}
