import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$.element($$renderer, tag, () => {
		$$renderer.push(`${$.attr_class('svelte-tsvhash', void 0, { active: on })}`);
	});
}
