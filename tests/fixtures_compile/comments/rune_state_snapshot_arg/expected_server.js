import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let s = { x: 1 };
	let snap = /* frozen */ s;
	function set() {
		snap = null;
	}
	$$renderer.push(`<button>${$.escape(snap)}</button>`);
}
