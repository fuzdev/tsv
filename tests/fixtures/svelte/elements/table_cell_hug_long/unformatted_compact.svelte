<!-- td at exactly 100 chars - fits on one line -->
<td><a href="x">xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a></td>

<!-- td at 101 chars - hug mode: moves only final > to new line -->
<td><a href="x">xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a></td>

<!-- td at 107 chars - full td hug: <td then ><a...></td then > -->
<td><a href="x">xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a></td>

<!-- td at 109 chars - cascading hug: inner a also breaks -->
<td><a href="x">xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a></td>

<!-- th at 101 chars - same hug behavior -->
<th><a href="x">xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a></th>

<!-- tr at 101 chars - td inside tr uses hug mode -->
<tr><td><a>xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a></td></tr>

<!-- tbody at 101 chars - nested tr/td use hug mode -->
<tbody><tr><td><a>xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a></td></tr></tbody>

<!-- thead at 101 chars -->
<thead><tr><th><a>xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a></th></tr></thead>

<!-- tfoot at 101 chars -->
<tfoot><tr><td><a>xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a></td></tr></tfoot>
