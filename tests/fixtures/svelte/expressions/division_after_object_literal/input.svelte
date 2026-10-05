<!-- an object literal's `}` ends an operand, so the `/` after it divides -->
<div>{{} / 2}</div>
<div>{{ a: 1 } / 2}</div>
<div>{x + {} / 2}</div>
<div>{c ? {} / 2 : d}</div>
<div>{c ? d : {} / 2}</div>
<div>{[{} / 2]}</div>
<div data-attr={{} / 2}></div>
<div data-attr="a {{} / 2}"></div>
<div style:color={{} / 2}></div>
<div {...{} / 2}></div>
{#if {} / 2}text{/if}
{#each {} / 2 as item}text{/each}
{#each items as item ({} / 2)}text{/each}
{#key {} / 2}text{/key}
{#snippet fn(a = {} / 2)}{a}{/snippet}
<div>{`${{} / 2}`}</div>

<!-- a block's `}` ends none, so the `/` after it opens a regex -->
<div>
	{(() => {
		if (c) {
		}
		/}/.test(a);
	})()}
</div>
<div>
	{(() => {
		function fn() {}
		/}/.test(a);
	})()}
</div>
