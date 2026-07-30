# bypass = 認証なしで通す設定。この policy を消すと policy ゼロ = 全拒否になり、公開している status page が落ちる
resource "cloudflare_zero_trust_access_policy" "status_page" {
  account_id = local.account_id
  name       = "Everyone"
  decision   = "bypass"

  include = [{
    everyone = {}
  }]

  # self_hosted には無意味だが live が持っている値。外すと apply で Cloudflare 側から消える
  connection_rules = {
    rdp = {}
  }
}

resource "cloudflare_zero_trust_access_policy" "grafana" {
  account_id = local.account_id
  name       = "only me"
  decision   = "allow"

  include = [{
    email = {
      email = "justicesparrow13@gmail.com"
    }
  }]

  connection_rules = {
    rdp = {}
  }
}

resource "cloudflare_zero_trust_access_application" "status_page" {
  account_id = local.account_id
  name       = "bons8i"
  domain     = "bons8i.hagaspa.com"
  type       = "self_hosted"

  session_duration           = "24h"
  auto_redirect_to_identity  = false
  app_launcher_visible       = true
  enable_binding_cookie      = false
  http_only_cookie_attribute = false
  options_preflight_bypass   = false

  policies = [{
    id         = cloudflare_zero_trust_access_policy.status_page.id
    precedence = 1
  }]
}

resource "cloudflare_zero_trust_access_application" "grafana" {
  account_id = local.account_id
  name       = "grafana"
  domain     = "grafana.hagaspa.com"
  type       = "self_hosted"

  session_duration           = "24h"
  allowed_idps               = [local.github_idp_id]
  auto_redirect_to_identity  = false
  app_launcher_visible       = true
  enable_binding_cookie      = false
  http_only_cookie_attribute = false
  options_preflight_bypass   = false

  policies = [{
    id         = cloudflare_zero_trust_access_policy.grafana.id
    precedence = 1
  }]
}
