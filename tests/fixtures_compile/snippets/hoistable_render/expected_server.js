import * as $ from 'svelte/internal/server';
function row($$renderer, label, value) {
	$$renderer.push(`<li>${$.escape(label)}: ${$.escape(value)}</li>`);
}
export default function Input($$renderer) {
	row($$renderer, a, 1);
}
