data "archive_file" "seed" {
  type        = "zip"
  source_file = "${path.module}/lambda/external-probe/index.mjs"
  output_path = "${path.module}/.build/seed.zip"
}

resource "aws_cloudwatch_log_group" "probe" {
  name              = "/aws/lambda/bons8i-external-probe"
  retention_in_days = 14
}

resource "aws_lambda_function" "probe" {
  function_name = "bons8i-external-probe"
  runtime       = "nodejs24.x"
  architectures = ["arm64"]
  memory_size   = 128
  timeout       = 120
  handler       = "index.handler"
  filename      = data.archive_file.seed.output_path
  role          = aws_iam_role.probe.arn

  environment {
    variables = {
      TARGET_URL     = "https://bons8i.hagaspa.com/"
      GITHUB_REPO    = "HagaSpa/bons8i"
      PAT_PARAM_NAME = local.github_pat_param_name
    }
  }

  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }

  depends_on = [aws_cloudwatch_log_group.probe]
}
