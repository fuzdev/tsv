import * as $ from 'svelte/internal/server';
function row($$renderer, value, prefix = '>') {
	$$renderer.push(`<li>${$.escape(prefix)}${$.escape(value)}</li>`);
}
export default function Input($$renderer, $$props) {
	let { label } = $$props;
	$$renderer.push(`<ul>`);
	row($$renderer, label);
	$$renderer.push(`<!----></ul>`);
}
