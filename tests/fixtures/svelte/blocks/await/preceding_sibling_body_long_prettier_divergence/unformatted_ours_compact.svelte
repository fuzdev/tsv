<!-- await after an expression-tag sibling at the 100/101 boundary: the head is short and stays on one line, but the body still drops at 101. At 100 the whole construct stays inline. Prettier hugs the } and breaks the component internally (prettier_variant_hug). No inline-element > to dangle here. -->
<div>
<!-- inline form == 100: stays inline -->
{x}{#await promise then value}<Comp prop1={value1} prop2={value2} prop3={value3xxxxxxx}/>{/await}

<!-- inline form == 101: body drops, the short head stays on one line -->
{x}{#await promise then value}<Comp prop1={value1} prop2={value2} prop3={value3xxxxxxxx}/>{/await}
</div>
