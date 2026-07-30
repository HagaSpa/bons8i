# import 済みなので通常は no-op。tfstate を失ったときの復旧経路なので消さない
# （消すと Terraform が新規作成を試み、Access application が二重になる）
# application の ID は accounts/ プレフィックスあり、policy はなし（形式が違う）
import {
  to = cloudflare_zero_trust_access_application.status_page
  id = "accounts/2a87a79cf0893b13e9ffa4b1340373e9/a6cbc9fc-6f03-4488-aeab-e8a99e137ddf"
}

import {
  to = cloudflare_zero_trust_access_application.grafana
  id = "accounts/2a87a79cf0893b13e9ffa4b1340373e9/d216043f-54ec-41e9-a14b-9a5c540bfdd3"
}

import {
  to = cloudflare_zero_trust_access_policy.status_page
  id = "2a87a79cf0893b13e9ffa4b1340373e9/d1aa0eef-8be8-4555-a709-9f1ce326a6d3"
}

import {
  to = cloudflare_zero_trust_access_policy.grafana
  id = "2a87a79cf0893b13e9ffa4b1340373e9/7774fb85-f11e-4647-bb4c-8f086b2da1d9"
}
