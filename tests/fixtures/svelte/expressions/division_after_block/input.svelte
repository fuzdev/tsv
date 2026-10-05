<script lang="ts"></script>

<!-- a statement's `}` ends no operand, so the `/` after it opens a regex -->
<div>
	{(() => {
		a;
		{
		}
		/}/.test(s);
	})()}
</div>
<div>
	{(() => {
		a = b;
		{
		}
		/}/.test(s);
	})()}
</div>
<div>
	{(() => {
		function fn(): void {}
		/}/.test(s);
	})()}
</div>
<div>
	{(() => {
		function fn(): string {}
		/[{]/.test(s);
	})()}
</div>
<div>
	{(() => {
		enum E {
			A
		}
		/}/.test(s);
	})()}
</div>
<div>
	{(() => {
		interface I {}
		/}/.test(s);
	})()}
</div>
<div>
	{(() => {
		namespace N {}
		/}/.test(s);
	})()}
</div>
<div>
	{(() => {
		type T = {};
		/}/.test(s);
	})()}
</div>
<div>
	{function () {
		a;
		{
		}
		/}/.test(s);
	}}
</div>
<div>
	{x +
		function () {
			a;
			{
			}
			/}/.test(s);
		}}
</div>
