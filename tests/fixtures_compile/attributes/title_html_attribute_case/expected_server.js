import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$$renderer.push(`<title data-x="y">hi</title>`);
}
