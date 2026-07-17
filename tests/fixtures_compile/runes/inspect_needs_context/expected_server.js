import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let x = 0;
		let d = new Date();
		$$renderer.push(`<!---->0
${$.escape(d.getFullYear())}`);
	});
}
