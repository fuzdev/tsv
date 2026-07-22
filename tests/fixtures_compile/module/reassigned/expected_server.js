import * as $ from 'svelte/internal/server';
let cnt = 0;
function bump() {
	cnt += 1;
}
export default function Input($$renderer) {
	$$renderer.push(`<p>${$.escape(cnt)}</p>`);
}
