<!-- a `/` after an object literal's `}` divides -->
{#if   x + {}  /  2}text1/}{/if}
{#if   { a: 1 }  /  2}text1/}{/if}
{#if   cond}text1{:else  if	x + {}  /  2}text2/}{/if}
{#if   cond}text1{:else  if	{ a: 1 }  /  2}text2/}{/if}
{#key	x + {}  /  2}text/}{/key}
{#key	{ a: 1 }  /  2}text/}{/key}
<div>{@html  x + {}  /  2}/}</div>
<div>{@html  { a: 1 }  /  2}/}</div>
{@render   fn(x + {}  /  2)}/}
{@render   fn({ a: 1 }  /  2)}/}
<div {@attach
	x + {}  /  2}>/}</div>
<div {@attach
	{ a: 1 }  /  2}>/}</div>
<div {...  x + {}  /  2}>/}</div>
<div {...  { a: 1 }  /  2}>/}</div>

<!-- a `/` after a block's `}` opens a regex -->
{#if   fn(() => {
	if (a) {
	}
	/}/.test(b);
})}
	text1/}
{/if}
{#if   cond}
	text1
{:else  if	fn(() => {
	if (a) {
	}
	/}/.test(b);
})}
	text2/}
{/if}
{#key	fn(() => {
	if (a) {
	}
	/}/.test(b);
})}
	text/}
{/key}
<div>
	{@html  fn(() => {
		if (a) {
		}
		/}/.test(b);
	})}/}
</div>
{@render   fn(() => {
	if (a) {
	}
	/}/.test(b);
})}/}
<div
	{@attach
	fn(() => {
		if (a) {
		}
		/}/.test(b);
	})}
>
	/}
</div>
<div
	{...  fn(() => {
		if (a) {
		}
		/}/.test(b);
	})}
>
	/}
</div>
