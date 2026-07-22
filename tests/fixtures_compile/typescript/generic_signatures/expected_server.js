import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	function first(list) {
		return list[0];
	}
	const pick = (list, index) => list[index];
	let a = first([1, 2]);
	let b = pick([3, 4], 1);
	$$renderer.push(`<p>${$.escape(a)}${$.escape(b)}</p>`);
}
