<script lang="ts"></script>

<!-- a postfix `!` ends no operand of its own: the operand before it does, so the `/` divides -->
<div>{a! / 2}</div>
<div>{a.b! / 2}</div>
<div>{f()! / 2}</div>
<div>{a!! / 2}</div>
<div>{a /* c */! / 2}</div>
<div data-attr={a! / 2}></div>
{#if a! / 2}text{/if}

<!-- a prefix `!` starts none: the operator before it governs, so the `/` opens a regex -->
<div>{!/}/.test(a)}</div>
<div>{a && !/}/.test(b)}</div>
{#if !/}/.test(a)}text{/if}
<div>
	{(() => {
		if (c)!/}/.test(a);
	})()}
</div>
<div>
	{(() => {
		a;
		!/}/.test(b);
	})()}
</div>
<div>
	{(() => {
		a; // c
		!/}/.test(b);
	})()}
</div>
<div>
	{(() => {
		return a; // c
		!/}/.test(b);
	})()}
</div>
<div>
	{(() => {
		a;
		/* c
		 */ !/}/.test(b);
	})()}
</div>
<div>
	{(() => {
		a;
		/** doc
		 */ !/}/.test(b);
	})()}
</div>
<div>
	{(() => {
		if (/\(/.test(s))!/}/.test(t);
	})()}
</div>
<div>
	{(() => {
		if (/\)/.test(s))!/}/.test(t);
	})()}
</div>
