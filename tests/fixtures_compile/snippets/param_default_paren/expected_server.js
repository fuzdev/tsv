import * as $ from 'svelte/internal/server';
function fn1($$renderer, a = (x + y) * 2) {
	$$renderer.push(`<span>${$.escape(a)}</span>`);
}
function fn2($$renderer, a = -(x ** 2)) {
	$$renderer.push(`<span>${$.escape(a)}</span>`);
}
function fn3($$renderer, a = () => ({})) {
	$$renderer.push(`<span>${$.escape(a)}</span>`);
}
export default function Input($$renderer) {
	fn1($$renderer);
	$$renderer.push(`<!----> `);
	fn2($$renderer);
	$$renderer.push(`<!----> `);
	fn3($$renderer);
	$$renderer.push(`<!---->`);
}
