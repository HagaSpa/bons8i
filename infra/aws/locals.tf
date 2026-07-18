locals {
  aws_region            = "ap-northeast-1"
  github_pat_param_name = "/bons8i/monitoring/alertmanager-to-github/github-token"
  backup_bucket         = "bons8i-backup" # created outside terraform (also used by etcd backups)
}
