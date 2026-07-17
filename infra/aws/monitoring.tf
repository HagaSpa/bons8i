resource "aws_sns_topic" "probe_errors" {
  name = "bons8i-external-probe-errors"
}

resource "aws_cloudwatch_metric_alarm" "probe_errors" {
  alarm_name          = "bons8i-external-probe-errors"
  namespace           = "AWS/Lambda"
  metric_name         = "Errors"
  statistic           = "Sum"
  period              = 600
  evaluation_periods  = 1
  threshold           = 1
  comparison_operator = "GreaterThanOrEqualToThreshold"
  treat_missing_data  = "notBreaching"

  dimensions = {
    FunctionName = aws_lambda_function.probe.function_name
  }

  alarm_actions = [aws_sns_topic.probe_errors.arn]
  ok_actions    = [aws_sns_topic.probe_errors.arn]
}

resource "aws_chatbot_slack_channel_configuration" "probe" {
  configuration_name = "bons8i-external-probe"
  iam_role_arn       = aws_iam_role.chatbot.arn
  slack_team_id      = "T0BHP71R75W"
  slack_channel_id   = "C0BHP72451N"
  sns_topic_arns     = [aws_sns_topic.probe_errors.arn]
}
