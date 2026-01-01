<!-- Attrs fit on one line, expression child on its own line -->
<div>
	<button type="button" class="plain selectable width_100" onclick={() => init(template)}>
		{template.name}
	</button>
</div>

<!-- Block child (should work correctly) -->
<div>
	<button type="button" class="plain selectable width_100" onclick={() => init(template)}>
		<div>{template.name}</div>
	</button>
</div>
