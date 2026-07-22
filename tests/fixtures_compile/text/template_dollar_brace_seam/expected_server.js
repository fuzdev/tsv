import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let user = 'root';
	$$renderer.push(`<code>ssh \${DEPLOY_USER}@root</code>`);
}
