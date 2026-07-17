terraform {
  backend "s3" {
    bucket       = "bons8i-tfstate"
    key          = "aws.tfstate"
    region       = "ap-northeast-1"
    use_lockfile = true
  }
}
