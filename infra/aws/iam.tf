data "aws_caller_identity" "current" {}

resource "aws_iam_role" "probe" {
  name = "bons8i-external-probe"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect    = "Allow"
      Principal = { Service = "lambda.amazonaws.com" }
      Action    = "sts:AssumeRole"
    }]
  })
}

resource "aws_iam_role_policy" "probe" {
  name = "bons8i-external-probe"
  role = aws_iam_role.probe.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Allow"
        Action   = ["logs:CreateLogStream", "logs:PutLogEvents"]
        Resource = "${aws_cloudwatch_log_group.probe.arn}:*"
      },
      {
        Effect   = "Allow"
        Action   = "ssm:GetParameter"
        Resource = "arn:aws:ssm:${local.aws_region}:${data.aws_caller_identity.current.account_id}:parameter${local.github_pat_param_name}"
      }
    ]
  })
}

resource "aws_iam_role" "scheduler" {
  name = "bons8i-external-probe-scheduler"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect    = "Allow"
      Principal = { Service = "scheduler.amazonaws.com" }
      Action    = "sts:AssumeRole"
    }]
  })
}

resource "aws_iam_role_policy" "scheduler" {
  name = "bons8i-external-probe-scheduler"
  role = aws_iam_role.scheduler.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = "lambda:InvokeFunction"
      Resource = aws_lambda_function.probe.arn
    }]
  })
}

resource "aws_iam_role" "chatbot" {
  name = "bons8i-external-probe-chatbot"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect    = "Allow"
      Principal = { Service = "chatbot.amazonaws.com" }
      Action    = "sts:AssumeRole"
    }]
  })
}

resource "aws_iam_role_policy" "chatbot" {
  name = "bons8i-external-probe-chatbot"
  role = aws_iam_role.chatbot.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["cloudwatch:Describe*", "cloudwatch:Get*", "cloudwatch:List*"]
      Resource = "*"
    }]
  })
}

# IAM user for in-cluster backup jobs (vmbackup mirror + monthly metrics archive).
# Access keys are issued manually and stored in SSM (never in terraform state).
resource "aws_iam_user" "vmbackup" {
  name = "bons8i-vmbackup"
}

resource "aws_iam_user_policy" "vmbackup" {
  name = "bons8i-vmbackup"
  user = aws_iam_user.vmbackup.name

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Allow"
        Action   = "s3:ListBucket"
        Resource = "arn:aws:s3:::${local.backup_bucket}"
        Condition = {
          StringLike = {
            "s3:prefix" = ["victoria-metrics/*", "vm-archive/*"]
          }
        }
      },
      {
        Effect = "Allow"
        Action = ["s3:GetObject", "s3:PutObject", "s3:DeleteObject"]
        Resource = [
          "arn:aws:s3:::${local.backup_bucket}/victoria-metrics/*",
          "arn:aws:s3:::${local.backup_bucket}/vm-archive/*",
        ]
      }
    ]
  })
}
