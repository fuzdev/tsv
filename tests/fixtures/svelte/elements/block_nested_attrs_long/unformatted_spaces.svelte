<!-- 96 chars at root - no wrap -->
<span   class="x"   data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"   ></span>

<!-- 97 chars at root - no wrap -->
<span   class="x"   data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"   ></span>

<!-- 96 + 2 = 98 effective at indent 1 - no wrap -->
<div>
	<span   class="x"   data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"   ></span>
</div>

<!-- 97 + 2 = 99 effective at indent 1 - no wrap -->
<div>
	<span   class="x"   data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"   ></span>
</div>

<!-- 98 + 2 = 100 effective at indent 1 - no wrap (exactly 100) -->
<div>
	<span   class="x"   data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"   ></span>
</div>

<!-- 99 + 2 = 101 effective at indent 1 - WRAPS (exceeds 100) -->
<div>
	<span   class="x"   data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"   ></span>
</div>

<!-- 96 + 4 = 100 effective at indent 2 - no wrap (exactly 100) -->
<div>
	<section>
		<span   class="x"   data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"   ></span>
	</section>
</div>

<!-- 97 + 4 = 101 effective at indent 2 - WRAPS (exceeds 100) -->
<div>
	<section>
		<span   class="x"   data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"   ></span>
	</section>
</div>

<!-- 92 + 6 = 98 effective at indent 3 - no wrap -->
<div>
	<section>
		<article>
			<span   class="x"   data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"   ></span>
		</article>
	</section>
</div>

<!-- 93 + 6 = 99 effective at indent 3 - no wrap -->
<div>
	<section>
		<article>
			<span   class="x"   data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"   ></span>
		</article>
	</section>
</div>

<!-- Inline hug mode: attr line 101 at col 0 - HUG MODE (attrs inline, > on new line) -->
<span   class="x"   data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"   ></span>

<!-- Inline full multiline: attr line 102 at col 0 - FULL MULTILINE (each attr on own line) -->
<span   class="x"   data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"   ></span>

<!-- Nested void element: 101 effective at indent 1 - WRAPS -->
<div>
	<input   type="text"   placeholder="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"   />
</div>

<!-- Nested block element: 101 effective at indent 1 - WRAPS (no hug mode for block) -->
<div>
	<p   class="x"   data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"   ></p>
</div>
