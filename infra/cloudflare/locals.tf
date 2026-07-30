locals {
  account_id = "2a87a79cf0893b13e9ffa4b1340373e9"

  # IdP は Terraform 管理外（import すると OAuth client_secret が tfstate に入る）。ID 参照のみ
  github_idp_id = "b1b6451a-abb6-499c-9e77-dab6d714a24b"
}
