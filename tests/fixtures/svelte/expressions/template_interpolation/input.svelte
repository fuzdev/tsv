<!-- Basic interpolation -->
<div data-x={`a ${b} c`}></div>

<!-- Object property in interpolation -->
<div data-x={`value: ${obj.x}`}></div>

<!-- Expression with braces in interpolation -->
<div data-x={`items: ${items.map((i) => i.name)}`}></div>

<!-- Nested template literals -->
<div data-x={`outer ${`inner ${x}`}`}></div>
