import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let v = '';
	let c = false;
	let g = 'a';
	let obj = { x: 1 };
	let x = 1;
	let el = void 0;
	$$renderer.push(`<input type="radio"${$.attr('checked', g === 'a', true)} value="a"/>`);
}
