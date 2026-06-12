<!-- Textarea attrs inside hugged label - closing suffix shouldn't force attr wrapping -->

<!-- 81 chars: attrs + suffix = 100, fits -->
<div>
	<div>
		<label>
			<textarea data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"></textarea></label
		>
	</div>
</div>

<!-- 82 chars: attrs + suffix = 101, attrs stay inline with > on next line -->
<div>
	<div>
		<label>
			<textarea data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
			></textarea></label
		>
	</div>
</div>
