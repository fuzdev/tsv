import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = 1;
	let b = 2;
	$$renderer.push(`<p>${$.escape(b)}</p> <button>x</button>`);
}
