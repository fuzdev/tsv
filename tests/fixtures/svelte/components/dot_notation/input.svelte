<!-- Uppercase last segment — component-cased -->
<ns.Comp />

<ns.Comp prop="value" />

<deep.nested.Comp />

<!-- Lowercase last segment — still a component (any dotted tag is member access) -->
<Object.component />

<object.property />

<!-- Component with children -->
<ns.Comp>
	<span>text</span>
</ns.Comp>
