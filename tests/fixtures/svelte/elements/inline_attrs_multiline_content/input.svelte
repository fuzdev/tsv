<!-- Inline element with multiline attrs and text content -->
<div>
	<button
		type="button"
		class="class1 class2 class3 class4 class5 class6 class7 class8"
		onclick={() => fn(arg1, arg2)}
	>
		text
	</button>
</div>

<!-- Inline element with multiline attrs and inline child -->
<div>
	<span
		class="class1 class2 class3 class4 class5 class6 class7 class8"
		data-attr="value1 value2 value3"
	>
		<em>text</em>
	</span>
</div>

<!-- Inline anchor with multiline attrs -->
<div>
	<a
		href="/path/to/resource"
		class="class1 class2 class3 class4 class5 class6 class7 class8"
		onclick={fn}
	>
		text
	</a>
</div>

<!-- Multiple inline children in multiline inline parent -->
<div>
	<label for="id" class="class1 class2 class3 class4 class5 class6 class7 class8" data-attr="value">
		<input type="checkbox" />
		text
	</label>
</div>
