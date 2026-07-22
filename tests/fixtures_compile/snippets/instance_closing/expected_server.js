import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { name } = $$props;
	function greeting($$renderer) {
		$$renderer.push(`<p>Hello, ${$.escape(name)}!</p>`);
	}
	greeting($$renderer);
}
