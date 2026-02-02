<!-- Test: member expression breaking at print width boundary -->

<!-- 100 chars exactly - no break needed -->
<div><section><table><tbody>{#each items as item}<tr><td><a href={fn(`/${aa.bb.ccccccxyyyy}`)}>{aa.bb.ccccccxyyyy}{#if aa.bb.ddddddxyy.eeeeee}text{aa.bb.ddddddxyy.eeeeee}{/if}</a></td></tr>{/each}</tbody></table></section></div>

<!-- 101 chars - member expression breaks to stay within print width -->
<div><section><table><tbody>{#each items as item}<tr><td><a href={fn(`/${aa.bb.ccccccxyyyyy}`)}>{aa.bb.ccccccxyyyyy}{#if aa.bb.ddddddxyy.eeeeee}text{aa.bb.ddddddxyy.eeeeee}{/if}</a></td></tr>{/each}</tbody></table></section></div>
