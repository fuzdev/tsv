<!-- a whitespace-only run between value parts re-indents to the attribute's column -->
<div>
<div>
<div style:transform-origin="{expr1}
{expr2}"></div>
</div>
</div>

<!-- a blank line in the run is preserved as one blank -->
<div style:transform-origin="{expr1}

{expr2}"></div>

<!-- the run at the value's leading edge -->
<div style:transform-origin="
{expr1}"></div>

<!-- the run at the value's trailing edge -->
<div style:transform-origin="{expr1}
"></div>

<!-- contrast: a plain attribute value is opaque, so its continuation keeps the authored indent -->
<div data-attr="{expr1}
      {expr2}"></div>
