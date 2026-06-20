<!-- A deeply-nested {#if} (5 levels) whose component body overflows the hugged head line, so the body drops; at this depth the dropped component meets the 100/101 boundary itself (100 stays one line, 101 wraps its attributes). Prettier hugs the } and breaks the component internally (prettier_variant_hug). -->
<div><div><div><div><div>
<!-- dropped component at body indent == 100: stays on one line -->
{#if cond}<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5abcde}/>{/if}

<!-- dropped component at body indent > 100 (101): wraps its own attributes -->
{#if cond}<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5abcdef}/>{/if}
</div></div></div></div></div>
