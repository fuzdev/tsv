<script lang="ts"></script>

<!-- a `>` closing a type-argument list ends an operand, so the `/` after it divides -->
<div>{f<T> / 2}</div>
<div>{a.b<T> / 2}</div>
<div>{f<A<B>> / 2}</div>
<div>{f()<T> / 2}</div>
<div data-attr={f<T> / 2}></div>
<div data-attr="a {f<T> / 2}"></div>
{#if f<T> / 2}text{/if}
<div>{@html f<T> / 2}</div>
{#each items as item (f<T> / 2)}text{/each}
<div>{`${f<T> / 2}`}</div>

<!-- a comparison, an arrow or an angle-bracket assertion ends no operand, so the `/` after it opens a regex -->
<div>{a > /}/.test(b)}</div>
<div>{a < b && c > /}/.test(d)}</div>
<div>{(s) => /}/.test(s)}</div>
<div>{a && <RegExp>/}/.source}</div>
{#if <RegExp>/}/.source}text{/if}
<div>
	{(() => {
		if (/\)/.test(s)) <RegExp>/}/.source;
	})()}
</div>
