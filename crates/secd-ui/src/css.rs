//! Token-only layout glue. No chrome, no palette.

pub const APP_CSS: &str = "\
.secd-auth-form{display:flex;flex-direction:column;gap:16px;width:100%}\
.secd-btn-block{width:100%;justify-content:center}\
.secd-stack{display:flex;flex-direction:column;gap:16px}\
.secd-row{display:flex;align-items:center;gap:8px;flex-wrap:wrap}\
.secd-grid-2{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:16px}\
.secd-list{display:flex;flex-direction:column;gap:8px}\
.secd-field-actions{display:flex;align-items:center;gap:8px;margin-top:8px}\
@media (max-width:900px){.secd-grid-2{grid-template-columns:minmax(0,1fr)}}\
";
